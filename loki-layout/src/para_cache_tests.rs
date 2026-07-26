// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for `para_cache` (extracted for the 300-line ceiling).

use crate::color::LayoutColor;
use crate::font::FontResources;
use crate::para::{ResolvedParaProps, StyleSpan, layout_paragraph};

fn resources() -> FontResources {
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
        character_border: None,
        letter_spacing: None,
        font_variant: None,
        word_spacing: None,
        shadow: false,
        emboss: false,
        imprint: false,
        link_url: None,
        math: None,
        scale: None,
        kerning: None,
        baseline_shift: None,
        language: None,
    }
}

fn lay(r: &mut FontResources, text: &str, spans: &[StyleSpan], width: f32) {
    let _ = layout_paragraph(
        r,
        text,
        spans,
        &ResolvedParaProps::default(),
        width,
        1.0,
        true,
    );
}

#[test]
fn identical_inputs_hit_and_match() {
    let mut r = resources();
    let text = "Hello cache world";
    let spans = [span(text)];

    let first = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        true,
    );
    assert_eq!(
        r.para_cache.len(),
        1,
        "first call should populate the cache"
    );

    let second = layout_paragraph(
        &mut r,
        text,
        &spans,
        &ResolvedParaProps::default(),
        400.0,
        1.0,
        true,
    );
    // Identical inputs must be a hit (no new entry) and reproduce the layout.
    assert_eq!(
        r.para_cache.len(),
        1,
        "identical call should hit, not insert"
    );
    assert_eq!(first.height, second.height);
    assert_eq!(first.width, second.width);
    assert_eq!(first.items.len(), second.items.len());
}

#[test]
fn changed_inputs_are_misses() {
    let mut r = resources();
    let base = "alpha";

    lay(&mut r, base, &[span(base)], 400.0);
    assert_eq!(r.para_cache.len(), 1);

    // Different text.
    lay(&mut r, "bravo", &[span("bravo")], 400.0);
    assert_eq!(r.para_cache.len(), 2, "different text must miss");

    // Different width, same text/spans.
    lay(&mut r, base, &[span(base)], 200.0);
    assert_eq!(r.para_cache.len(), 3, "different width must miss");

    // Different char property (bold) on the same text.
    let mut bold = span(base);
    bold.bold = true;
    lay(&mut r, base, &[bold], 400.0);
    assert_eq!(r.para_cache.len(), 4, "different style span must miss");

    // Different run language (gap #30): squiggle routing depends on it, so
    // it must participate in the key (covered by the Debug fold).
    let mut tagged = span(base);
    tagged.language = Some("fr-FR".into());
    lay(&mut r, base, &[tagged], 400.0);
    assert_eq!(r.para_cache.len(), 5, "different language must miss");
}

#[test]
fn clear_drops_all_entries() {
    let mut r = resources();
    lay(&mut r, "one", &[span("one")], 400.0);
    lay(&mut r, "two", &[span("two")], 400.0);
    assert_eq!(r.para_cache.len(), 2);

    r.clear_paragraph_cache();
    assert_eq!(r.para_cache.len(), 0, "clear should drop every entry");

    // A subsequent layout repopulates from scratch (miss, not stale hit).
    lay(&mut r, "one", &[span("one")], 400.0);
    assert_eq!(r.para_cache.len(), 1);
}

#[test]
fn preserve_flag_is_part_of_key() {
    let mut r = resources();
    let text = "preserve flag";
    let spans = [span(text)];
    let props = ResolvedParaProps::default();

    let _ = layout_paragraph(&mut r, text, &spans, &props, 400.0, 1.0, true);
    let _ = layout_paragraph(&mut r, text, &spans, &props, 400.0, 1.0, false);
    assert_eq!(
        r.para_cache.len(),
        2,
        "preserve_for_editing must distinguish cache entries"
    );
}
