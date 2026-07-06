// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::*;

#[test]
fn prev_grapheme_in_ascii_mid() {
    // "hello": h(0) e(1) l(2) l(3) o(4)
    assert_eq!(prev_grapheme_boundary("hello", 3), 2);
}

#[test]
fn prev_grapheme_at_start() {
    assert_eq!(prev_grapheme_boundary("hello", 0), 0);
}

#[test]
fn next_grapheme_in_ascii_mid() {
    assert_eq!(next_grapheme_boundary("hello", 2), 3);
}

#[test]
fn next_grapheme_at_end() {
    assert_eq!(next_grapheme_boundary("hello", 5), 5);
}

#[test]
fn prev_grapheme_multibyte() {
    // "héllo": h(0) é(1..3) l(3) l(4) o(5)
    // é is U+00E9, encoded as 0xC3 0xA9 — 2 bytes.
    // byte 3 is the start of 'l'; prev boundary should be 1 (start of é).
    let s = "héllo";
    assert_eq!(prev_grapheme_boundary(s, 3), 1);
}

#[test]
fn next_grapheme_emoji() {
    // "a😀b": a(0) 😀(1..5) b(5)
    // 😀 is U+1F600, encoded as 4 bytes.
    // next boundary after offset 1 should be 5 (end of emoji / start of 'b').
    let s = "a\u{1F600}b";
    assert_eq!(next_grapheme_boundary(s, 1), 5);
}

#[test]
fn top_level_position_block_path_is_flat() {
    let pos = DocumentPosition::top_level(0, 3, 7);
    assert_eq!(pos.block_path(), BlockPath::block(3));
    assert!(pos.path.is_empty());
}

#[test]
fn nested_position_block_path_carries_steps() {
    let pos = DocumentPosition {
        page_index: 0,
        paragraph_index: 2,
        byte_offset: 0,
        path: vec![PathStep::Cell { cell: 1, block: 0 }],
    };
    assert_eq!(pos.block_path(), BlockPath::in_cell(2, 1, 0));
}

#[test]
fn sibling_block_shifts_top_level_paragraph() {
    let pos = DocumentPosition::top_level(0, 3, 7);
    let next = pos.sibling_block(1, 0);
    assert_eq!(next.paragraph_index, 4);
    assert_eq!(next.byte_offset, 0);
    assert!(next.path.is_empty());
    // Saturates at 0 rather than underflowing.
    assert_eq!(
        DocumentPosition::top_level(0, 0, 0)
            .sibling_block(-1, 5)
            .paragraph_index,
        0
    );
}

#[test]
fn sibling_block_shifts_nested_leaf_block_only() {
    let pos = DocumentPosition {
        page_index: 0,
        paragraph_index: 2,
        byte_offset: 9,
        path: vec![PathStep::Cell { cell: 1, block: 0 }],
    };
    let next = pos.sibling_block(1, 0);
    // Root paragraph_index is untouched; the leaf block index advances.
    assert_eq!(next.paragraph_index, 2);
    assert_eq!(next.byte_offset, 0);
    assert_eq!(next.path, vec![PathStep::Cell { cell: 1, block: 1 }]);
}

#[test]
fn has_selection_ignores_page_index_only_differences() {
    // A page-spanning paragraph fragment can give the same logical caret a
    // different page_index on each endpoint; that is NOT a range selection.
    let mut cs = CursorState::new();
    cs.anchor = Some(DocumentPosition::top_level(0, 4, 7));
    cs.focus = Some(DocumentPosition::top_level(1, 4, 7));
    assert!(
        !cs.has_selection(),
        "endpoints differing only in page_index must not count as a selection"
    );
    // A genuine byte difference is still a selection.
    cs.focus = Some(DocumentPosition::top_level(1, 4, 8));
    assert!(
        cs.has_selection(),
        "a real byte-range must count as a selection"
    );
}

#[test]
fn cursor_block_path_follows_focus() {
    let mut cs = CursorState::new();
    assert_eq!(cs.block_path(), None);
    cs.focus = Some(DocumentPosition {
        page_index: 0,
        paragraph_index: 5,
        byte_offset: 0,
        path: vec![PathStep::Note { note: 0, block: 0 }],
    });
    assert_eq!(cs.block_path(), Some(BlockPath::in_note(5, 0, 0)));
}
