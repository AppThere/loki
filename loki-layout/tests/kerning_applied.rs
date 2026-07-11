// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Regression locks for shaping features (fidelity gap #23).
//!
//! Kerning was listed as an open fidelity gap from the swash-shaped Parley
//! era; the Parley 0.10 upgrade (harfrust shaper) closed it silently, which
//! the 2026-07-05 conformance calibration initially mis-attributed. These
//! tests pin the *verified-current* behaviour — GPOS pair kerning and
//! standard ligatures are applied — so a future Parley upgrade or a shaping
//! feature change cannot silently regress them again.
//!
//! Ground truth (from Carlito-Regular's tables, upem 2048):
//! - `A` advance 1185 units = 13.886 pt @ 24 pt
//! - kern pair (A,V) = −89 units = −1.043 pt @ 24 pt
//! - `f`+`i` (625+470) vs the `fi` ligature (1084 units)

use loki_layout::{
    FontResources, LayoutColor, PositionedItem, ResolvedParaProps, StyleSpan, layout_paragraph,
};

fn carlito_span(text: &str, font_size: f32, kerning: Option<bool>) -> StyleSpan {
    StyleSpan {
        range: 0..text.len(),
        font_name: Some("Carlito".into()),
        font_size,
        bold: false,
        weight: 400,
        italic: false,
        color: LayoutColor::BLACK,
        underline: None,
        strikethrough: None,
        line_height: None,
        vertical_align: None,
        highlight_color: None,
        letter_spacing: None,
        font_variant: None,
        word_spacing: None,
        shadow: false,
        link_url: None,
        math: None,
        scale: None,
        kerning,
        baseline_shift: None,
        language: None,
    }
}

/// Lays out `text` in Carlito and returns the first glyph run's advances.
fn advances(text: &str, font_size: f32, kerning: Option<bool>) -> Vec<f32> {
    let mut resources = FontResources::new();
    for blob in loki_fonts::fallback_font_blobs() {
        resources.register_font(blob.to_vec());
    }
    let para = layout_paragraph(
        &mut resources,
        text,
        &[carlito_span(text, font_size, kerning)],
        &ResolvedParaProps::default(),
        1000.0,
        1.0,
        false,
    );
    for item in &para.items {
        if let PositionedItem::GlyphRun(run) = item {
            return run.glyphs.iter().map(|g| g.advance).collect();
        }
    }
    panic!("no glyph run produced for {text:?}");
}

/// With kerning enabled, "AV"'s A carries the (A,V) kern: its effective
/// advance is the natural advance (13.886 pt @ 24 pt) minus 1.043 pt.
#[test]
fn gpos_pair_kerning_is_applied_when_enabled() {
    let adv = advances("AV", 24.0, Some(true));
    assert_eq!(adv.len(), 2, "AV must shape to two glyphs");
    let natural_a = 1185.0 * 24.0 / 2048.0; // 13.886
    let kerned_a = (1185.0 - 89.0) * 24.0 / 2048.0; // 12.844
    assert!(
        (adv[0] - kerned_a).abs() < 0.05,
        "A before V must carry the (A,V) kern pair: expected ≈{kerned_a:.3}, got {} \
         (unkerned would be {natural_a:.3} — if you see that, shaping lost kerning)",
        adv[0]
    );
}

/// The default (no explicit kerning property) must NOT kern — matching the
/// reference apps: Word's `w:kern` threshold defaults to 0 (off) and
/// LibreOffice treats an ODT without `style:letter-kerning` as off. This is
/// what keeps loki's line widths aligned with reference renders of documents
/// that never asked for kerning (see goldens/CALIBRATION.md).
#[test]
fn kerning_defaults_off_like_the_reference_apps() {
    let adv = advances("AV", 24.0, None);
    let natural_a = 1185.0 * 24.0 / 2048.0;
    assert!(
        (adv[0] - natural_a).abs() < 0.05,
        "A must keep its natural advance {natural_a:.3} when kerning is not \
         requested, got {}",
        adv[0]
    );
}

/// The kern must be contextual, not a blanket advance change: a bare "A"
/// keeps its natural advance.
#[test]
fn kerning_is_contextual() {
    let adv = advances("AH", 24.0, Some(true)); // no (A,H) kern pair in Carlito
    let natural_a = 1185.0 * 24.0 / 2048.0;
    assert!(
        (adv[0] - natural_a).abs() < 0.05,
        "A before H must keep its natural advance {natural_a:.3}, got {}",
        adv[0]
    );
}

/// Standard ligatures must be applied: "five" shapes to 3 glyphs (fi + v + e).
#[test]
fn standard_ligatures_are_applied() {
    let adv = advances("five", 24.0, None);
    assert_eq!(
        adv.len(),
        3,
        "'five' must shape with the fi ligature (3 glyphs), got {} glyphs",
        adv.len()
    );
}
