// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for tracked-change run styling.

use super::*;
use loki_doc_model::style::props::revision::RevisionMark;

/// A plain black span with no decorations — the starting point `apply` mutates.
fn bare_span() -> StyleSpan {
    StyleSpan {
        range: 0..1,
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
        kerning: None,
        link_url: None,
        math: None,
        scale: None,
        baseline_shift: None,
        language: None,
    }
}

fn props_with(rev: Option<RevisionMark>) -> CharProps {
    CharProps {
        revision: rev,
        ..CharProps::default()
    }
}

#[test]
fn insertion_underlines_and_recolours() {
    let mut span = bare_span();
    apply(
        &mut span,
        &props_with(Some(RevisionMark::new(RevisionKind::Insertion))),
    );
    assert!(span.underline.is_some(), "insertion is underlined");
    assert!(span.strikethrough.is_none());
    assert_ne!(span.color, LayoutColor::BLACK, "recoloured for the author");
}

#[test]
fn deletion_strikes_through_and_recolours() {
    let mut span = bare_span();
    apply(
        &mut span,
        &props_with(Some(RevisionMark::new(RevisionKind::Deletion))),
    );
    assert!(span.strikethrough.is_some(), "deletion is struck through");
    assert!(span.underline.is_none());
    assert_ne!(span.color, LayoutColor::BLACK);
}

#[test]
fn no_revision_leaves_the_span_untouched() {
    let mut span = bare_span();
    apply(&mut span, &props_with(None));
    assert!(span.underline.is_none() && span.strikethrough.is_none());
    assert_eq!(span.color, LayoutColor::BLACK);
}

#[test]
fn same_author_gets_a_stable_colour_distinct_from_another() {
    let ada = author_color(Some("Ada"));
    assert_eq!(ada, author_color(Some("Ada")), "deterministic per author");
    // Not asserting inequality for arbitrary names (hash collisions are allowed),
    // but these two chosen names land on different palette entries.
    assert_ne!(author_color(Some("Ada")), author_color(Some("Bob")));
}
