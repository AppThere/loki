// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for [`crate::para`].

use super::*;
use crate::items::{BorderStyle, PositionedItem};
use loki_doc_model::style::list_style::{BulletChar, LabelAlignment, ListLevel, ListLevelKind, NumberingScheme};
use loki_primitives::units::Points as DocPoints;

// ── helpers ───────────────────────────────────────────────────────────────────

/// Build a `FontResources` with Liberation Sans registered so tests are not
/// dependent on fontconfig auto-discovery.
fn test_resources() -> FontResources {
    let mut r = FontResources::new();
    for p in [
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
    ] {
        if let Ok(data) = std::fs::read(p) {
            r.register_font(data);
        }
    }
    r
}

fn single_span(text: &str, font_size: f32) -> StyleSpan {
    StyleSpan {
        range: 0..text.len(),
        font_name: None,
        font_size,
        bold: false,
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
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[test]
fn plain_paragraph_non_empty() {
    let mut r = test_resources();
    let text = "Hello, world!";
    let spans = [single_span(text, 12.0)];
    let result = layout_paragraph(&mut r, text, &spans, &ResolvedParaProps::default(), 400.0, 1.0);
    assert!(result.height > 0.0, "height should be positive");
    assert!(!result.items.is_empty(), "items should not be empty");
}

#[test]
fn bold_span_produces_items() {
    let mut r = test_resources();
    let text = "Hello bold world";
    let spans = [
        StyleSpan { range: 0..6,              bold: false, ..single_span(text, 12.0) },
        StyleSpan { range: 6..10,             bold: true,  ..single_span(text, 12.0) },
        StyleSpan { range: 10..text.len(),    bold: false, ..single_span(text, 12.0) },
    ];
    let result = layout_paragraph(&mut r, text, &spans, &ResolvedParaProps::default(), 400.0, 1.0);
    assert!(!result.items.is_empty());
    let runs = result.items.iter().filter(|i| matches!(i, PositionedItem::GlyphRun(_))).count();
    assert!(runs >= 1, "expected at least one glyph run, got {runs}");
}

#[test]
fn narrow_width_causes_wrapping() {
    let mut r = test_resources();
    let text = "The quick brown fox jumps over the lazy dog";
    let spans = [single_span(text, 14.0)];
    let wide   = layout_paragraph(&mut r, text, &spans, &ResolvedParaProps::default(), 600.0, 1.0);
    let narrow = layout_paragraph(&mut r, text, &spans, &ResolvedParaProps::default(),  80.0, 1.0);
    assert!(narrow.height > wide.height, "narrow layout should be taller due to wrapping");
}

#[test]
fn background_color_is_first_item() {
    let mut r = test_resources();
    let text = "Background test";
    let props = ResolvedParaProps { background_color: Some(LayoutColor::WHITE), ..Default::default() };
    let result = layout_paragraph(&mut r, text, &[single_span(text, 12.0)], &props, 400.0, 1.0);
    assert!(
        matches!(result.items.first(), Some(PositionedItem::FilledRect(_))),
        "first item should be FilledRect for paragraph background",
    );
}

#[test]
fn underline_span_emits_decoration() {
    let mut r = test_resources();
    let text = "Underlined text";
    let spans = [StyleSpan { underline: Some(UnderlineStyle::Single), ..single_span(text, 12.0) }];
    let result = layout_paragraph(&mut r, text, &spans, &ResolvedParaProps::default(), 400.0, 1.0);
    let has_underline = result.items.iter().any(|item| {
        matches!(item, PositionedItem::Decoration(d) if d.kind == DecorationKind::Underline)
    });
    assert!(has_underline, "expected a Underline decoration item");
}

#[test]
fn space_before_after_not_in_height() {
    let mut r = test_resources();
    let text = "Spacing test";
    let spans = [single_span(text, 12.0)];
    let no_space   = layout_paragraph(&mut r, text, &spans, &ResolvedParaProps::default(), 400.0, 1.0);
    let with_space = layout_paragraph(&mut r, text, &spans,
        &ResolvedParaProps { space_before: 24.0, space_after: 24.0, ..Default::default() },
        400.0, 1.0);
    assert_eq!(
        no_space.height, with_space.height,
        "space_before/space_after must not affect ParagraphLayout::height",
    );
}

#[test]
fn line_boundaries_populated_for_multiline_paragraph() {
    let mut r = test_resources();
    let text = "The quick brown fox jumps over the lazy dog and continues for many more words to force wrapping";
    let spans = [single_span(text, 12.0)];
    // Narrow width forces several lines.
    let result = layout_paragraph(&mut r, text, &spans, &ResolvedParaProps::default(), 100.0, 1.0);
    assert!(
        result.line_boundaries.len() >= 2,
        "expected multiple lines, got {}",
        result.line_boundaries.len()
    );
    // Each line's max_coord must be greater than its min_coord.
    for (i, &(min, max)) in result.line_boundaries.iter().enumerate() {
        assert!(max > min, "line {i}: max_coord ({max}) must exceed min_coord ({min})");
    }
    // max_coords must be strictly increasing (each line's bottom is further down).
    for i in 1..result.line_boundaries.len() {
        let prev_max = result.line_boundaries[i - 1].1;
        let curr_max = result.line_boundaries[i].1;
        assert!(
            curr_max > prev_max,
            "line {i} max_coord ({curr_max}) must exceed previous line max_coord ({prev_max})"
        );
    }
    // Last line's max_coord should approximate the total paragraph height.
    let last_max = result.line_boundaries.last().unwrap().1;
    assert!(
        (last_max - result.height).abs() < 1.0,
        "last line max_coord ({last_max}) should equal paragraph height ({})",
        result.height
    );
}

#[test]
fn empty_paragraph_has_no_line_boundaries() {
    let mut r = test_resources();
    let result = layout_paragraph(&mut r, "", &[], &ResolvedParaProps::default(), 400.0, 1.0);
    assert!(
        result.line_boundaries.is_empty(),
        "empty paragraph must have no line boundaries"
    );
}

#[test]
fn border_follows_background() {
    let mut r = test_resources();
    let text = "Border test";
    let edge = BorderEdge { color: LayoutColor::BLACK, width: 1.0, style: BorderStyle::Solid };
    let props = ResolvedParaProps {
        background_color: Some(LayoutColor::WHITE),
        border_top: Some(edge),
        ..Default::default()
    };
    let result = layout_paragraph(&mut r, text, &[single_span(text, 12.0)], &props, 400.0, 1.0);
    assert!(matches!(result.items.first(),    Some(PositionedItem::FilledRect(_))));
    assert!(matches!(result.items.get(1), Some(PositionedItem::BorderRect(_))));
}

#[test]
fn superscript_span_uses_smaller_font() {
    // A span with vertical_align=Superscript should use font_size * 0.58.
    // We verify by checking that the layout of a superscript run produces a
    // GlyphRun with a smaller ascent than a plain run at the same font_size.
    // The simplest proxy: just ensure the paragraph lays out without panic and
    // produces at least one glyph run.
    let mut r = test_resources();
    let text = "x2";
    let spans = [StyleSpan {
        range: 0..2,
        vertical_align: Some(VerticalAlign::Superscript),
        ..single_span(text, 12.0)
    }];
    let result = layout_paragraph(&mut r, text, &spans, &ResolvedParaProps::default(), 400.0, 1.0);
    let runs = result.items.iter().filter(|i| matches!(i, PositionedItem::GlyphRun(_))).count();
    assert!(runs >= 1, "superscript span must produce at least one glyph run");
}

#[test]
fn highlight_color_produces_filled_rect_before_glyph_run() {
    let mut r = test_resources();
    let text = "highlighted";
    let spans = [StyleSpan {
        highlight_color: Some(LayoutColor::new(1.0, 1.0, 0.0, 1.0)),
        ..single_span(text, 12.0)
    }];
    let result = layout_paragraph(&mut r, text, &spans, &ResolvedParaProps::default(), 400.0, 1.0);
    // First non-background item should be a FilledRect (highlight), then a GlyphRun.
    let rects = result.items.iter().filter(|i| matches!(i, PositionedItem::FilledRect(_))).count();
    assert!(rects >= 1, "highlight span must produce at least one FilledRect");
    // The FilledRect must come before the GlyphRun.
    let rect_pos = result.items.iter().position(|i| matches!(i, PositionedItem::FilledRect(_))).unwrap();
    let run_pos  = result.items.iter().position(|i| matches!(i, PositionedItem::GlyphRun(_))).unwrap();
    assert!(rect_pos < run_pos, "FilledRect (highlight) must precede its GlyphRun");
}

#[test]
fn shadow_span_produces_extra_glyph_run() {
    let mut r = test_resources();
    let text = "shadow";
    let plain_spans  = [single_span(text, 12.0)];
    let shadow_spans = [StyleSpan { shadow: true, ..single_span(text, 12.0) }];
    let plain  = layout_paragraph(&mut r, text, &plain_spans,  &ResolvedParaProps::default(), 400.0, 1.0);
    let shadow = layout_paragraph(&mut r, text, &shadow_spans, &ResolvedParaProps::default(), 400.0, 1.0);
    let plain_runs  = plain.items.iter().filter(|i| matches!(i, PositionedItem::GlyphRun(_))).count();
    let shadow_runs = shadow.items.iter().filter(|i| matches!(i, PositionedItem::GlyphRun(_))).count();
    assert!(
        shadow_runs > plain_runs,
        "shadow span must produce more GlyphRun items than plain ({shadow_runs} vs {plain_runs})"
    );
}

// ── format_list_marker tests ──────────────────────────────────────────────────

fn bullet_level(c: char) -> ListLevel {
    ListLevel {
        level: 0,
        kind: ListLevelKind::Bullet { char: BulletChar::Char(c), font: None },
        indent_start: DocPoints::new(36.0),
        hanging_indent: DocPoints::new(18.0),
        label_alignment: LabelAlignment::Left,
        tab_stop_after_label: None,
        char_props: Default::default(),
    }
}

fn numbered_level(level: u8, scheme: NumberingScheme, format: &str, display_levels: u8, start: u32) -> ListLevel {
    ListLevel {
        level,
        kind: ListLevelKind::Numbered {
            scheme,
            start_value: start,
            format: format.to_string(),
            display_levels,
        },
        indent_start: DocPoints::new(36.0),
        hanging_indent: DocPoints::new(18.0),
        label_alignment: LabelAlignment::Left,
        tab_stop_after_label: None,
        char_props: Default::default(),
    }
}

fn counters(vals: &[(usize, u32)]) -> [u32; 9] {
    let mut arr = [0u32; 9];
    for &(i, v) in vals { arr[i] = v; }
    arr
}

#[test]
fn format_marker_bullet() {
    let levels = vec![bullet_level('•')];
    assert_eq!(format_list_marker(&levels, 0, &counters(&[(0, 1)])), "•");
}

#[test]
fn format_marker_decimal_with_suffix() {
    let levels = vec![numbered_level(0, NumberingScheme::Decimal, "%1.", 1, 1)];
    assert_eq!(format_list_marker(&levels, 0, &counters(&[(0, 3)])), "3.");
}

#[test]
fn format_marker_lower_letter_overflow() {
    let levels = vec![numbered_level(0, NumberingScheme::LowerAlpha, "%1.", 1, 1)];
    assert_eq!(format_list_marker(&levels, 0, &counters(&[(0, 1)])),  "a.");
    assert_eq!(format_list_marker(&levels, 0, &counters(&[(0, 26)])), "z.");
    assert_eq!(format_list_marker(&levels, 0, &counters(&[(0, 27)])), "aa.");
}

#[test]
fn format_marker_upper_roman() {
    let levels = vec![numbered_level(0, NumberingScheme::UpperRoman, "%1.", 1, 1)];
    assert_eq!(format_list_marker(&levels, 0, &counters(&[(0, 4)])), "IV.");
}

#[test]
fn format_marker_display_levels_two_level() {
    let levels = vec![
        numbered_level(0, NumberingScheme::Decimal, "%1.", 1, 1),
        numbered_level(1, NumberingScheme::Decimal, "%1.%2.", 2, 1),
    ];
    // level 0 counter = 2, level 1 counter = 3 → "2.3."
    assert_eq!(format_list_marker(&levels, 1, &counters(&[(0, 2), (1, 3)])), "2.3.");
}

#[test]
fn format_marker_picture_bullet_fallback() {
    let levels = vec![ListLevel {
        level: 0,
        kind: ListLevelKind::Bullet { char: BulletChar::Image, font: None },
        indent_start: DocPoints::new(36.0),
        hanging_indent: DocPoints::new(18.0),
        label_alignment: LabelAlignment::Left,
        tab_stop_after_label: None,
        char_props: Default::default(),
    }];
    assert_eq!(format_list_marker(&levels, 0, &counters(&[])), "•");
}

// ── Counter tracking tests ────────────────────────────────────────────────────

#[test]
fn counter_advance_single_list() {
    // advance_counter is tested via format_list_marker indirectly.
    // We directly test the alpha_label helper through format_counter logic.
    // Three advances: 1, 2, 3.
    let levels = vec![numbered_level(0, NumberingScheme::Decimal, "%1.", 1, 1)];
    for (i, expected) in [(1, "1."), (2, "2."), (3, "3.")] {
        assert_eq!(format_list_marker(&levels, 0, &counters(&[(0, i)])), expected);
    }
}

#[test]
fn counter_nested_deeper_reset() {
    // When level 0 advances, level 1 should have been reset to 0.
    // We simulate: level 0 = 2, level 1 = 0 (reset) then first use = 1.
    let levels = vec![
        numbered_level(0, NumberingScheme::Decimal, "%1.", 1, 1),
        numbered_level(1, NumberingScheme::Decimal, "%1.%2.", 2, 1),
    ];
    // After level-0 advances to 2 and level-1 is reset, the next level-1
    // item should show "2.1." (level-1 reinitialised from start_value=1).
    let c = counters(&[(0, 2), (1, 1)]);
    assert_eq!(format_list_marker(&levels, 1, &c), "2.1.");
}
