// SPDX-License-Identifier: Apache-2.0

//! Tests for font-size grow/shrink: the pure ladder stepping and the
//! end-to-end mark application over a selection.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::get_mark_at;
use loki_doc_model::loro_bridge::document_to_loro;
use loki_doc_model::loro_schema::MARK_FONT_SIZE_PT;
use loro::{LoroDoc, LoroValue};

use super::{adjust_font_size, grow, shrink};
use crate::editing::cursor::{CursorState, DocumentPosition};

fn loro_with(text: &str) -> LoroDoc {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(vec![Inline::Str(text.into())])];
    document_to_loro(&doc).expect("to loro")
}

fn selection(start: usize, end: usize) -> CursorState {
    let mut cs = CursorState::new();
    cs.anchor = Some(DocumentPosition::top_level(0, 0, start));
    cs.focus = Some(DocumentPosition::top_level(0, 0, end));
    cs
}

fn size_at(loro: &LoroDoc, byte: usize) -> Option<f64> {
    match get_mark_at(loro, 0, byte, MARK_FONT_SIZE_PT).expect("get_mark_at") {
        Some(LoroValue::Double(v)) => Some(v),
        _ => None,
    }
}

#[test]
fn grow_steps_up_the_ladder() {
    assert_eq!(grow(11.0), 12.0);
    assert_eq!(grow(12.0), 14.0);
    assert_eq!(grow(8.0), 9.0);
    assert_eq!(grow(96.0), 96.0, "clamped at the top");
    assert_eq!(grow(500.0), 96.0);
}

#[test]
fn shrink_steps_down_the_ladder() {
    assert_eq!(shrink(12.0), 11.0);
    assert_eq!(shrink(11.0), 10.5);
    assert_eq!(shrink(9.0), 8.0);
    assert_eq!(shrink(8.0), 8.0, "clamped at the bottom");
}

#[test]
fn off_ladder_size_snaps_to_the_neighbour() {
    // 13 pt is not on the ladder: grow → 14, shrink → 12.
    assert_eq!(grow(13.0), 14.0);
    assert_eq!(shrink(13.0), 12.0);
}

#[test]
fn adjust_applies_a_size_mark_over_the_selection_only() {
    let loro = loro_with("hello world");
    // Select "hello" (0..5). No direct size → default 11 → grow → 12.
    adjust_font_size(&loro, &selection(0, 5), true).expect("grow");
    assert_eq!(
        size_at(&loro, 2),
        Some(12.0),
        "grown to 12pt inside selection"
    );
    assert_eq!(size_at(&loro, 8), None, "untouched outside the selection");
}

#[test]
fn grow_then_shrink_returns_to_the_start() {
    let loro = loro_with("hello world");
    adjust_font_size(&loro, &selection(0, 5), true).expect("grow"); // 11 → 12
    adjust_font_size(&loro, &selection(0, 5), false).expect("shrink"); // 12 → 11
    assert_eq!(size_at(&loro, 2), Some(11.0));
}
