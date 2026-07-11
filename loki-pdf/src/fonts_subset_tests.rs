// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the pure subsetting helpers (tag derivation + CIDToGIDMap).
//! The full subset round-trip on a real font is covered by the crate's
//! integration test (`embeds_fonts_and_output_intent`).

use super::{cid_to_gid_map, subset_tag};
use std::collections::BTreeSet;
use subsetter::GlyphRemapper;

#[test]
fn subset_tag_is_six_uppercase_letters() {
    let used: BTreeSet<u16> = [5u16, 7, 42].into_iter().collect();
    let tag = subset_tag(&used);
    assert_eq!(tag.len(), 6);
    assert!(
        tag.iter().all(|b| b.is_ascii_uppercase()),
        "tag must be A–Z: {tag:?}"
    );
}

#[test]
fn subset_tag_is_deterministic_and_set_sensitive() {
    let a: BTreeSet<u16> = [5u16, 7, 42].into_iter().collect();
    let b: BTreeSet<u16> = [5u16, 7, 42].into_iter().collect();
    let c: BTreeSet<u16> = [5u16, 7, 43].into_iter().collect();
    assert_eq!(subset_tag(&a), subset_tag(&b), "same set ⇒ same tag");
    assert_ne!(
        subset_tag(&a),
        subset_tag(&c),
        "different set ⇒ different tag"
    );
}

#[test]
fn cid_to_gid_map_remaps_each_used_glyph() {
    // Remapper assigns contiguous ids: .notdef=0 already present, then 5→1, 9→2.
    let used: BTreeSet<u16> = [5u16, 9].into_iter().collect();
    let mut remapper = GlyphRemapper::new();
    for &g in &used {
        remapper.remap(g);
    }
    let map = cid_to_gid_map(&used, &remapper);
    // Length spans CID 0..=max_used (9) ⇒ 10 entries × 2 bytes.
    assert_eq!(map.len(), 20);
    let gid_at = |cid: usize| u16::from(map[cid * 2]) << 8 | u16::from(map[cid * 2 + 1]);
    assert_eq!(gid_at(5), remapper.get(5).unwrap(), "CID 5 → subset gid");
    assert_eq!(gid_at(9), remapper.get(9).unwrap(), "CID 9 → subset gid");
    // An unused CID maps to .notdef (0).
    assert_eq!(gid_at(3), 0, "unused CID ⇒ .notdef");
}
