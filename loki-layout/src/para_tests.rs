// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for [`crate::para`].

use super::*;
use crate::items::{BorderStyle, PositionedItem};

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
        underline: false,
        strikethrough: false,
        line_height: None,
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
    let spans = [StyleSpan { underline: true, ..single_span(text, 12.0) }];
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
