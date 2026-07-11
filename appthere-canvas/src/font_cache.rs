// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Cache for [`peniko::FontData`] instances keyed by pointer identity.
//!
//! Gate with the `font-cache` feature.

use std::collections::HashMap;
use std::sync::Arc;

/// Cache that maps raw font bytes (pointer address + index) to [`peniko::FontData`].
///
/// Construct one per rendering context and reuse across frames to avoid
/// redundant Blob allocations.
#[cfg(feature = "font-cache")]
#[derive(Default)]
pub struct FontDataCache {
    entries: HashMap<(usize, u32), peniko::FontData>,
    coords: HashMap<(usize, u32), Vec<i16>>,
}

#[cfg(feature = "font-cache")]
impl FontDataCache {
    /// Creates an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a [`peniko::FontData`] for the given font bytes and collection index.
    ///
    /// Two `Arc<Vec<u8>>` values pointing to the same allocation share a cache
    /// entry; different allocations with identical bytes are treated as distinct.
    pub fn get_or_insert(&mut self, data: &Arc<Vec<u8>>, font_index: u32) -> &peniko::FontData {
        let key = (Arc::as_ptr(data) as usize, font_index);
        self.entries.entry(key).or_insert_with(|| {
            let mut bytes: Vec<u8> = (**data).clone();
            strip_bitmap_tables(&mut bytes);
            let blob = peniko::Blob::from(bytes);
            peniko::FontData::new(blob, font_index)
        })
    }

    /// Returns the cached normalized coordinates (defaulting to zero) for the
    /// font's axes, in `F2Dot14` bit representation — the form
    /// `DrawGlyphs::normalized_coords` consumes, so per-glyph-run callers need
    /// no conversion allocation.
    pub fn get_coords(&mut self, data: &Arc<Vec<u8>>, font_index: u32) -> &[i16] {
        let key = (Arc::as_ptr(data) as usize, font_index);
        self.coords.entry(key).or_insert_with(|| {
            if let Ok(font_ref) = read_fonts::FontRef::from_index(data, font_index) {
                use read_fonts::TableProvider;
                if let Ok(fvar) = font_ref.fvar() {
                    if let Ok(axes) = fvar.axes() {
                        // Default (all-zero) normalized position on every axis.
                        return vec![0i16; axes.len()];
                    }
                }
            }
            vec![]
        })
    }
}

fn strip_bitmap_tables(data: &mut [u8]) {
    // Check if it's a TTC collection
    if data.starts_with(b"ttcf") {
        if data.len() < 12 {
            return;
        }
        let num_fonts = u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize;
        let header_size = 12 + num_fonts * 4;
        if data.len() < header_size {
            return;
        }
        for i in 0..num_fonts {
            let offset_idx = 12 + i * 4;
            let offset = u32::from_be_bytes([
                data[offset_idx],
                data[offset_idx + 1],
                data[offset_idx + 2],
                data[offset_idx + 3],
            ]) as usize;
            if offset < data.len() {
                strip_sfnt_bitmap_tables(data, offset);
            }
        }
    } else {
        strip_sfnt_bitmap_tables(data, 0);
    }
}

fn strip_sfnt_bitmap_tables(data: &mut [u8], offset: usize) {
    if data.len() < offset + 12 {
        return;
    }
    let num_tables = u16::from_be_bytes([data[offset + 4], data[offset + 5]]) as usize;
    let directory_start = offset + 12;
    let directory_size = num_tables * 16;
    if data.len() < directory_start + directory_size {
        return;
    }
    for i in 0..num_tables {
        let entry_offset = directory_start + i * 16;
        let tag = &mut data[entry_offset..entry_offset + 4];
        if tag == b"EBLC"
            || tag == b"EBDT"
            || tag == b"EBSC"
            || tag == b"CBLC"
            || tag == b"CBDT"
            || tag == b"sbix"
        {
            // Rename to make them unrecognized (e.g. prefix with 'X')
            tag[0] = b'X';
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a minimal single-font sfnt: a 12-byte offset table (with
    /// `numTables` set) followed by one 16-byte directory record per tag.
    /// Only the tag bytes of each record carry meaning for the strip logic;
    /// the remaining 12 bytes per record are zero-filled.
    fn sfnt_with_tags(tags: &[&[u8; 4]]) -> Vec<u8> {
        let mut data = vec![0u8; 12];
        data[0..4].copy_from_slice(&[0x00, 0x01, 0x00, 0x00]); // sfnt version 1.0
        data[4..6].copy_from_slice(&(tags.len() as u16).to_be_bytes());
        for tag in tags {
            data.extend_from_slice(*tag);
            data.extend_from_slice(&[0u8; 12]); // checksum + offset + length
        }
        data
    }

    fn tag_at(data: &[u8], table_index: usize) -> [u8; 4] {
        let start = 12 + table_index * 16;
        data[start..start + 4].try_into().unwrap()
    }

    #[test]
    fn strips_each_bitmap_table_tag() {
        for tag in [b"EBLC", b"EBDT", b"EBSC", b"CBLC", b"CBDT", b"sbix"] {
            let mut data = sfnt_with_tags(&[tag]);
            strip_sfnt_bitmap_tables(&mut data, 0);
            assert_eq!(
                &tag_at(&data, 0),
                &[b'X', tag[1], tag[2], tag[3]],
                "first byte of {:?} should be renamed to 'X'",
                std::str::from_utf8(tag).unwrap(),
            );
        }
    }

    #[test]
    fn leaves_non_bitmap_tables_untouched() {
        let mut data = sfnt_with_tags(&[b"glyf", b"EBDT", b"cmap"]);
        strip_sfnt_bitmap_tables(&mut data, 0);
        assert_eq!(&tag_at(&data, 0), b"glyf");
        assert_eq!(&tag_at(&data, 1), b"XBDT"); // EBDT renamed
        assert_eq!(&tag_at(&data, 2), b"cmap");
    }

    #[test]
    fn truncated_buffers_do_not_panic() {
        // Shorter than the 12-byte offset table.
        let mut tiny = vec![0u8; 8];
        strip_sfnt_bitmap_tables(&mut tiny, 0);
        // Header claims 4 tables but the directory bytes are missing.
        let mut header_only = vec![0u8; 12];
        header_only[4..6].copy_from_slice(&4u16.to_be_bytes());
        strip_sfnt_bitmap_tables(&mut header_only, 0);
        strip_bitmap_tables(&mut header_only);
    }

    #[test]
    fn strips_inside_ttc_collection() {
        // Build one inner sfnt and wrap it in a `ttcf` header pointing at it.
        let inner = sfnt_with_tags(&[b"EBDT"]);
        let header_size = 12 + 4; // ttcf header + one 4-byte offset
        let mut data = vec![0u8; header_size];
        data[0..4].copy_from_slice(b"ttcf");
        data[8..12].copy_from_slice(&1u32.to_be_bytes()); // numFonts = 1
        data[12..16].copy_from_slice(&(header_size as u32).to_be_bytes()); // offset
        data.extend_from_slice(&inner);

        strip_bitmap_tables(&mut data);
        // The inner sfnt's EBDT tag (at header_size + 12) must be renamed.
        assert_eq!(&data[header_size + 12], &b'X');
    }

    #[cfg(feature = "font-cache")]
    #[test]
    fn get_coords_is_empty_for_non_font_bytes() {
        let mut cache = FontDataCache::new();
        let data = Arc::new(vec![0u8, 1, 2, 3]);
        assert!(cache.get_coords(&data, 0).is_empty());
    }

    #[cfg(feature = "font-cache")]
    #[test]
    fn get_or_insert_caches_by_pointer_identity() {
        let mut cache = FontDataCache::new();
        let a = Arc::new(vec![0u8; 4]);
        let b = Arc::new(vec![0u8; 4]); // identical bytes, distinct allocation
        cache.get_or_insert(&a, 0);
        cache.get_or_insert(&a, 0); // same Arc → reuses entry
        assert_eq!(cache.entries.len(), 1);
        cache.get_or_insert(&b, 0); // distinct allocation → new entry
        assert_eq!(cache.entries.len(), 2);
    }
}
