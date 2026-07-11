// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Glyph subsetting helpers for the PDF font embedder.
//!
//! [`subset_program`] reduces a font to the glyphs the document actually uses
//! (via the `subsetter` crate, which renumbers glyphs to a contiguous range),
//! [`cid_to_gid_map`] builds the `CIDToGIDMap` stream that maps the content
//! stream's original glyph ids (used as CIDs under Identity-H) to the subset's
//! renumbered gids, and [`subset_tag`] derives the mandatory six-letter subset
//! prefix for the embedded font name (PDF 32000-1 §9.6.4).

use std::collections::BTreeSet;

use subsetter::GlyphRemapper;

/// Subsets `data` to the used glyph ids. Returns the reduced font program and
/// the [`GlyphRemapper`] (old gid → new gid). On any subsetting failure (e.g. a
/// CFF2 or malformed face the subsetter rejects) it falls back to the full font
/// program and `None`, so export still produces a valid, fully-embedded font.
pub(super) fn subset_program(
    data: &[u8],
    index: u32,
    used: &BTreeSet<u16>,
) -> (Vec<u8>, Option<GlyphRemapper>) {
    let mut remapper = GlyphRemapper::new();
    for &gid in used {
        remapper.remap(gid);
    }
    match subsetter::subset(data, index, &remapper) {
        Ok(program) => (program, Some(remapper)),
        Err(_) => (data.to_vec(), None),
    }
}

/// Builds the `CIDToGIDMap` stream body: two big-endian bytes per CID from 0 to
/// the largest used glyph id, giving the subset gid for that CID (0/`.notdef`
/// for CIDs not in the subset). With Identity-H the content stream's CID equals
/// the original glyph id, so this remaps each to its position in the subset.
pub(super) fn cid_to_gid_map(used: &BTreeSet<u16>, remapper: &GlyphRemapper) -> Vec<u8> {
    let max_cid = used.iter().copied().max().unwrap_or(0);
    let mut map = vec![0u8; (usize::from(max_cid) + 1) * 2];
    for &old in used {
        let new = remapper.get(old).unwrap_or(0);
        let off = usize::from(old) * 2;
        map[off] = (new >> 8) as u8;
        map[off + 1] = (new & 0xff) as u8;
    }
    map
}

/// Derives the six-uppercase-letter subset tag required to prefix a subsetted
/// font's `BaseFont` name (`ABCDEF+Name`). Deterministic in the used-glyph set
/// (an FNV-1a hash mapped into A–Z), so an identical subset always tags the
/// same and distinct subsets almost never collide.
pub(super) fn subset_tag(used: &BTreeSet<u16>) -> [u8; 6] {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &gid in used {
        for byte in gid.to_be_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    let mut tag = [b'A'; 6];
    for slot in &mut tag {
        *slot = b'A' + (hash % 26) as u8;
        hash /= 26;
    }
    tag
}

#[cfg(test)]
#[path = "fonts_subset_tests.rs"]
mod tests;
