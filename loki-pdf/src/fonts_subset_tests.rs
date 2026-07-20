// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the pure subsetting helpers (tag derivation + CIDToGIDMap).
//! The full subset round-trip on a real font is covered by the crate's
//! integration test (`embeds_fonts_and_output_intent`).

use super::{cid_to_gid_map, subset_program, subset_tag};
use std::collections::BTreeSet;
use subsetter::GlyphRemapper;

/// Sums every outline control-point coordinate — a cheap fingerprint of a
/// glyph's shape that changes when the outline moves (e.g. under instancing).
#[derive(Default)]
struct PointSum(f64);
impl ttf_parser::OutlineBuilder for PointSum {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0 += f64::from(x) + f64::from(y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.0 += f64::from(x) + f64::from(y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.0 += f64::from(x1) + f64::from(y1) + f64::from(x) + f64::from(y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.0 += f64::from(x1) + f64::from(y1) + f64::from(x2) + f64::from(y2);
        self.0 += f64::from(x) + f64::from(y);
    }
    fn close(&mut self) {}
}

/// Instancing a variable font at a non-default weight must actually change the
/// embedded glyph outlines — the fix for bold Arial (Arimo `wght=700`) that
/// previously exported the default (regular) master with bold advances.
#[test]
fn variable_font_instancing_changes_glyph_outlines() {
    let wght = ttf_parser::Tag::from_bytes(b"wght");
    // The first bundled face with a `wght` axis is Arimo (the Arial substitute).
    let arimo = loki_fonts::fallback_font_blobs()
        .iter()
        .copied()
        .find(|b| {
            ttf_parser::Face::parse(b, 0)
                .map(|f| f.variation_axes().into_iter().any(|a| a.tag == wght))
                .unwrap_or(false)
        })
        .expect("a bundled variable font with a wght axis");

    let face = ttf_parser::Face::parse(arimo, 0).unwrap();
    let gid = face.glyph_index('B').expect("Arimo has 'B'").0;
    let used: BTreeSet<u16> = [0u16, gid].into_iter().collect();

    let (regular, rmap) = subset_program(arimo, 0, &used, &[]);
    let (bold, bmap) = subset_program(arimo, 0, &used, &[(wght, 700.0)]);
    // Same used set ⇒ deterministic remap ⇒ 'B' is at the same new gid in both.
    let new_gid = rmap.expect("regular subset ok").get(gid).unwrap();
    assert_eq!(
        new_gid,
        bmap.expect("bold subset ok").get(gid).unwrap(),
        "both subsets renumber identically"
    );

    let outline_sum = |program: &[u8]| {
        let f = ttf_parser::Face::parse(program, 0).expect("subset parses");
        let mut s = PointSum::default();
        f.outline_glyph(ttf_parser::GlyphId(new_gid), &mut s);
        s.0
    };
    let (r, b) = (outline_sum(&regular), outline_sum(&bold));
    assert!(
        (r - b).abs() > 1.0,
        "wght=700 must move the 'B' outline vs the regular master; regular={r} bold={b}"
    );
}

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
