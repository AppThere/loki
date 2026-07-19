// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for underline decorations across an underlined tab's expansion gap.

use super::{emit_tab_underline, tab_underline};
use crate::color::LayoutColor;
use crate::items::{DecorationKind, PositionedItem};
use crate::para::{StyleSpan, UnderlineStyle};

/// A minimal span at `range` with an optional underline, otherwise defaulted.
fn span(range: std::ops::Range<usize>, underline: Option<UnderlineStyle>) -> StyleSpan {
    StyleSpan {
        range,
        font_name: None,
        font_size: 12.0,
        bold: false,
        weight: 400,
        italic: false,
        color: LayoutColor::BLACK,
        underline,
        strikethrough: None,
        line_height: None,
        vertical_align: None,
        highlight_color: None,
        character_border: None,
        letter_spacing: None,
        font_variant: None,
        word_spacing: None,
        shadow: false,
        link_url: None,
        math: None,
        scale: None,
        kerning: None,
        baseline_shift: None,
        language: None,
    }
}

#[test]
fn zero_length_underlined_span_is_the_signature_tab() {
    // The classic signature tab: a tab-only run whose span collapsed to [p, p).
    let spans = [span(5..5, Some(UnderlineStyle::Single))];
    let found = tab_underline(&spans, 5).expect("underline recovered");
    assert_eq!(found.0, UnderlineStyle::Single);
}

#[test]
fn no_underline_returns_none() {
    let spans = [span(5..5, None), span(5..10, None)];
    assert!(tab_underline(&spans, 5).is_none());
}

#[test]
fn tab_inside_a_longer_underlined_run_is_covered() {
    // A tab embedded in underlined body text: the covering span carries it.
    let spans = [span(0..12, Some(UnderlineStyle::Double))];
    let found = tab_underline(&spans, 4).expect("underline recovered");
    assert_eq!(found.0, UnderlineStyle::Double);
}

#[test]
fn own_tab_run_wins_over_a_non_underlined_neighbour() {
    // The underlined tab run [5,5) must win over the following non-underlined
    // text run that also starts at 5 — otherwise the rule would be dropped.
    let spans = [span(5..5, Some(UnderlineStyle::Single)), span(5..9, None)];
    assert!(tab_underline(&spans, 5).is_some());
}

#[test]
fn emit_pushes_a_decoration_below_the_baseline() {
    let mut items = Vec::new();
    emit_tab_underline(
        &mut items,
        UnderlineStyle::Single,
        LayoutColor::BLACK,
        12.0,
        10.0,
        90.0,
        100.0,
    );
    assert_eq!(items.len(), 1);
    let PositionedItem::Decoration(d) = &items[0] else {
        panic!("expected a decoration");
    };
    assert_eq!(d.kind, DecorationKind::Underline);
    assert_eq!(d.x, 10.0);
    assert_eq!(d.width, 80.0);
    assert!(d.y > 100.0, "underline sits below the baseline");
}

#[test]
fn emit_skips_a_sub_pixel_gap() {
    let mut items = Vec::new();
    emit_tab_underline(
        &mut items,
        UnderlineStyle::Single,
        LayoutColor::BLACK,
        12.0,
        10.0,
        10.5,
        100.0,
    );
    assert!(items.is_empty(), "no rule for a <1pt gap");
}
