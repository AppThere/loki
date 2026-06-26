// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`crate::para_band`].

use super::*;
use crate::para::ResolvedParaProps;

fn test_resources() -> FontResources {
    let mut r = FontResources::new();
    for p in ["/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf"] {
        if let Ok(data) = std::fs::read(p) {
            r.register_font(data);
        }
    }
    r
}

fn span(text: &str) -> StyleSpan {
    StyleSpan {
        range: 0..text.len(),
        font_name: None,
        font_size: 12.0,
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
    }
}

/// Collect glyph-run `(y, x)` origins, sorted by y.
fn run_origins(body: &BandBody) -> Vec<(f32, f32)> {
    let mut v: Vec<(f32, f32)> = body
        .items
        .iter()
        .filter_map(|i| match i {
            PositionedItem::GlyphRun(g) => Some((g.origin.y, g.origin.x)),
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    v
}

#[test]
fn left_band_shifts_first_lines_and_tail_reclaims_full_width() {
    let mut r = test_resources();
    let text = "The quick brown fox jumps over the lazy dog. The quick brown fox jumps \
                over the lazy dog. The quick brown fox jumps over the lazy dog again \
                and again to fill several wrapped lines below the band region.";
    let spans = [span(text)];
    let props = ResolvedParaProps::default();
    // Left band: 60 pt wide, covering ~2 lines (cover_height 30 pt ≈ 2 × ~14 pt).
    let band = Band {
        inset: 60.0,
        cover_height: 30.0,
        shift_text: true,
    };
    let body = layout_band_body(&mut r, text, &spans, &props, 360.0, 1.0, band);

    let origins = run_origins(&body);
    assert!(origins.len() >= 4, "expected several wrapped lines");
    let first_y = origins.first().unwrap().0;
    let last_y = origins.last().unwrap().0;

    // Lines in the band (smallest y) are shifted right by the inset.
    let first_x = origins
        .iter()
        .filter(|(y, _)| (*y - first_y).abs() < 0.5)
        .map(|(_, x)| *x)
        .fold(f32::INFINITY, f32::min);
    assert!(
        (first_x - 60.0).abs() < 2.0,
        "band line should be shifted by the 60 pt inset; got {first_x}"
    );

    // Lines below the band reclaim the full column (x ≈ 0).
    let last_x = origins
        .iter()
        .filter(|(y, _)| (*y - last_y).abs() < 0.5)
        .map(|(_, x)| *x)
        .fold(f32::INFINITY, f32::min);
    assert!(
        last_x < 2.0,
        "tail line should reclaim the full left margin; got {last_x}"
    );
}

#[test]
fn right_band_narrows_without_shifting() {
    let mut r = test_resources();
    let text = "The quick brown fox jumps over the lazy dog repeatedly to fill several \
                lines of wrapped body text beside and below the right-hand band region.";
    let spans = [span(text)];
    let props = ResolvedParaProps::default();
    // Right band: text stays at the left edge (no shift), just narrowed.
    let band = Band {
        inset: 60.0,
        cover_height: 30.0,
        shift_text: false,
    };
    let body = layout_band_body(&mut r, text, &spans, &props, 360.0, 1.0, band);
    let origins = run_origins(&body);
    // All lines start at the left edge (x ≈ 0) regardless of the band.
    for (_, x) in &origins {
        assert!(*x < 2.0, "right band must not shift text; got x = {x}");
    }
}

#[test]
fn short_body_within_band_stays_single_segment() {
    let mut r = test_resources();
    let text = "Short text.";
    let spans = [span(text)];
    let props = ResolvedParaProps::default();
    let band = Band {
        inset: 40.0,
        cover_height: 200.0, // taller than the single line
        shift_text: true,
    };
    let body = layout_band_body(&mut r, text, &spans, &props, 360.0, 1.0, band);
    let origins = run_origins(&body);
    assert!(!origins.is_empty());
    // The single line is in the band → shifted right by the inset.
    for (_, x) in &origins {
        assert!(
            (*x - 40.0).abs() < 2.0,
            "in-band line should be shifted; got x = {x}"
        );
    }
}
