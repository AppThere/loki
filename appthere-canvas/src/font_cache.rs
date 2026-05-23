// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

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
    coords: HashMap<(usize, u32), Vec<read_fonts::types::F2Dot14>>,
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

    /// Returns the cached normalized coordinates (defaulting to zero) for the font's axes.
    pub fn get_coords(
        &mut self,
        data: &Arc<Vec<u8>>,
        font_index: u32,
    ) -> &[read_fonts::types::F2Dot14] {
        let key = (Arc::as_ptr(data) as usize, font_index);
        self.coords.entry(key).or_insert_with(|| {
            if let Ok(font_ref) = read_fonts::FontRef::from_index(&**data, font_index) {
                use read_fonts::TableProvider;
                if let Ok(fvar) = font_ref.fvar() {
                    if let Ok(axes) = fvar.axes() {
                        return vec![read_fonts::types::F2Dot14::default(); axes.len()];
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
