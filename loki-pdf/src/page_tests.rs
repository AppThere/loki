// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::*;
use loki_layout::{GlyphSynthesis, LayoutColor, LayoutPoint};
use std::sync::Arc;

fn run_with(ids: &[u16]) -> PositionedGlyphRun {
    PositionedGlyphRun {
        origin: LayoutPoint { x: 0.0, y: 0.0 },
        font_data: Arc::new(vec![0u8; 4]),
        font_index: 0,
        font_size: 12.0,
        glyphs: ids
            .iter()
            .map(|&id| GlyphEntry {
                id,
                x: 0.0,
                y: 0.0,
                advance: 6.0,
            })
            .collect(),
        color: LayoutColor::new(0.0, 0.0, 0.0, 1.0),
        synthesis: GlyphSynthesis::default(),
        link_url: None,
    }
}

// A run made only of .notdef (id 0) glyphs — e.g. the glyph Parley shapes
// for a tab character — must not register a face or emit any glyph, matching
// the on-screen `loki-vello` renderer. Previously these rendered as tofu.
#[test]
fn notdef_only_run_emits_nothing() {
    let mut bank = FontBank::new();
    let mut content = Content::new();
    render_run(&run_with(&[0, 0]), 100.0, 0.0, 0.0, &mut bank, &mut content);
    assert!(
        bank.is_empty(),
        "a .notdef-only run must not register a face"
    );
}

// A ClippedGroup must emit the PDF clip operators (`re` rect, `W` clip,
// `n` end-path) wrapped in save/restore, so table cell content is masked to
// the cell box rather than over-painting neighbours.
#[test]
fn clipped_group_emits_clip_operators() {
    use loki_layout::{LayoutRect, LayoutSize, PositionedRect};
    let mut fonts = FontBank::new();
    let mut images = ImageBank::new();
    let mut banks = PageBanks {
        fonts: &mut fonts,
        images: &mut images,
    };
    let mut content = Content::new();
    let child = PositionedItem::FilledRect(PositionedRect {
        rect: LayoutRect {
            origin: LayoutPoint { x: 10.0, y: 10.0 },
            size: LayoutSize {
                width: 5.0,
                height: 5.0,
            },
        },
        color: LayoutColor::new(0.0, 0.0, 0.0, 1.0),
    });
    let group = PositionedItem::ClippedGroup {
        clip_rect: LayoutRect {
            origin: LayoutPoint { x: 0.0, y: 0.0 },
            size: LayoutSize {
                width: 20.0,
                height: 20.0,
            },
        },
        items: vec![child],
    };
    render_item(&group, 100.0, 0.0, 0.0, &mut banks, &mut content);
    let bytes = content.finish().to_vec();
    let stream = String::from_utf8_lossy(&bytes);
    // `re` (rect) + `W` (clip-nonzero) + `n` (end-path) define the clip path;
    // `q`/`Q` bracket it so the clip is popped after the children paint.
    assert!(stream.contains("re"), "clip rect operator `re` missing");
    assert!(stream.contains('W'), "clip operator `W` missing");
    assert!(
        stream.contains('q') && stream.contains('Q'),
        "save/restore (`q`/`Q`) missing"
    );
}

// A run mixing .notdef with real glyphs registers the face but excludes the
// .notdef id from the subset (and never draws it).
#[test]
fn notdef_is_filtered_from_real_run() {
    let mut bank = FontBank::new();
    let mut content = Content::new();
    render_run(
        &run_with(&[0, 5, 0, 7]),
        100.0,
        0.0,
        0.0,
        &mut bank,
        &mut content,
    );
    assert_eq!(bank.faces().len(), 1);
    let ids = bank.used_glyph_ids(0);
    assert!(!ids.contains(&0), "the .notdef glyph must be filtered out");
    assert!(ids.contains(&5) && ids.contains(&7), "real glyphs kept");
}
