// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the caret content-space geometry.

use super::{PageStack, caret_line_height_px};

/// US Letter at 100%: 792 pt → 1056 CSS px, 24 px gap, 24 px container padding.
fn stack(zoom: f32) -> PageStack {
    PageStack {
        page_height_px: 1056.0,
        page_gap_px: 24.0,
        zoom,
        content_top_px: 24.0,
    }
}

#[test]
fn slot_scales_the_page_but_not_the_gap() {
    // The gap is a fixed CSS margin on the tile, so it must not be zoomed.
    // Getting this wrong drifts the caret by one gap per page — invisible on
    // page 1 and badly wrong on page 20.
    assert_eq!(stack(1.0).slot_px(), 1080.0);
    assert_eq!(stack(2.0).slot_px(), 2136.0);
    assert_eq!(stack(0.5).slot_px(), 552.0);
}

#[test]
fn first_page_starts_below_the_container_padding() {
    // Content y = 0 is the scroll container's top edge, not the first page's.
    assert_eq!(stack(1.0).content_y(0, 0.0), 24.0);
}

#[test]
fn later_pages_accumulate_whole_slots() {
    let s = stack(1.0);
    assert_eq!(s.content_y(1, 0.0), 24.0 + 1080.0);
    assert_eq!(s.content_y(4, 0.0), 24.0 + 4.0 * 1080.0);
}

#[test]
fn page_local_points_convert_at_ninety_six_over_seventy_two() {
    // 72 pt is one inch is 96 px at zoom 1.
    let s = stack(1.0);
    assert_eq!(s.content_y(0, 72.0), 24.0 + 96.0);
    assert_eq!(s.px_per_pt(), 96.0 / 72.0);
}

#[test]
fn zoom_scales_both_the_slot_and_the_in_page_offset() {
    let s = stack(2.0);
    // Page 1 top, plus a 72 pt margin inside it, both at 2x.
    assert_eq!(s.content_y(1, 72.0), 24.0 + 2136.0 + 192.0);
}

#[test]
fn line_height_prefers_the_caret_rect() {
    assert_eq!(
        caret_line_height_px(Some((0.0, 0.0, 1.0, 21.0)), 18.0),
        21.0
    );
}

#[test]
fn line_height_falls_back_when_the_caret_has_no_rect() {
    assert_eq!(caret_line_height_px(None, 18.0), 18.0);
    // A degenerate zero-height rect is not a usable line height either.
    assert_eq!(caret_line_height_px(Some((0.0, 0.0, 1.0, 0.0)), 18.0), 18.0);
}

#[test]
fn a_caret_on_a_later_page_is_far_down_the_content() {
    // Guards the whole chain: page 9, one inch into the page, at 100%.
    // 24 padding + 9 slots + 96 px = 9840.
    let s = stack(1.0);
    assert_eq!(s.content_y(9, 72.0), 24.0 + 9.0 * 1080.0 + 96.0);
}

// ── Reveal trigger (L08-019 / I-20) ──────────────────────────────────────────

use super::{CaretRevision, should_reveal};
use crate::editing::cursor::DocumentPosition;

fn pos(page: usize, para: usize, byte: usize) -> DocumentPosition {
    DocumentPosition {
        page_index: page,
        paragraph_index: para,
        byte_offset: byte,
        path: Vec::new(),
    }
}

#[test]
fn first_reveal_always_fires() {
    let now = CaretRevision::new(pos(0, 0, 0), None);
    assert!(should_reveal(None, &now));
}

#[test]
fn an_unchanged_caret_does_not_re_reveal() {
    // This is I-20 in miniature. The effect can re-run for any number of
    // reasons — a scroll event, a re-render, a props change — and none of them
    // moved the caret, so none of them may scroll the view.
    let rev = CaretRevision::new(pos(3, 12, 40), None);
    assert!(!should_reveal(Some(&rev), &rev.clone()));
}

#[test]
fn typing_a_character_fires() {
    let before = CaretRevision::new(pos(3, 12, 40), None);
    let after = CaretRevision::new(pos(3, 12, 41), None);
    assert!(should_reveal(Some(&before), &after));
}

#[test]
fn moving_to_another_page_fires() {
    let before = CaretRevision::new(pos(3, 12, 40), None);
    let after = CaretRevision::new(pos(4, 13, 0), None);
    assert!(should_reveal(Some(&before), &after));
}

#[test]
fn extending_a_selection_fires_even_when_the_focus_is_unchanged() {
    // Collapsing or extending at the same byte offset is still a caret change
    // the user made; without the anchor in the revision it would be invisible.
    let collapsed = CaretRevision::new(pos(1, 2, 10), None);
    let extended = CaretRevision::new(pos(1, 2, 10), Some(pos(1, 2, 4)));
    assert!(should_reveal(Some(&collapsed), &extended));
}

#[test]
fn entering_a_table_cell_fires() {
    // Same page, paragraph and offset, different container path.
    let outside = CaretRevision::new(pos(0, 5, 3), None);
    let mut inside_pos = pos(0, 5, 3);
    inside_pos.path = vec![loki_doc_model::PathStep::Cell { cell: 0, block: 0 }];
    let inside = CaretRevision::new(inside_pos, None);
    assert!(should_reveal(Some(&outside), &inside));
}
