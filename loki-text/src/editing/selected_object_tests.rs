// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`selected_object`] — the contextual-tab derivation (plan 4a.2).

use loki_doc_model::PathStep;

use super::{SelectedObject, selected_object};
use crate::editing::cursor::{CursorState, DocumentPosition};

fn cursor_at(pos: DocumentPosition) -> CursorState {
    let mut cs = CursorState::new();
    cs.focus = Some(pos.clone());
    cs.anchor = Some(pos);
    cs
}

#[test]
fn no_cursor_is_none() {
    assert_eq!(selected_object(&CursorState::new()), SelectedObject::None);
}

#[test]
fn top_level_paragraph_is_none() {
    let cs = cursor_at(DocumentPosition::top_level(0, 2, 0));
    assert_eq!(selected_object(&cs), SelectedObject::None);
}

#[test]
fn caret_in_a_table_cell_is_table() {
    let mut pos = DocumentPosition::top_level(0, 3, 0);
    pos.path = vec![PathStep::Cell { cell: 1, block: 0 }];
    assert_eq!(selected_object(&cursor_at(pos)), SelectedObject::Table);
}

#[test]
fn caret_in_a_note_body_is_none() {
    // Note bodies do not (yet) get a contextual tab.
    let mut pos = DocumentPosition::top_level(0, 3, 0);
    pos.path = vec![PathStep::Note { note: 0, block: 0 }];
    assert_eq!(selected_object(&cursor_at(pos)), SelectedObject::None);
}

#[test]
fn nested_table_reports_table() {
    // A cell step anywhere in the path (even under another cell) is a table.
    let mut pos = DocumentPosition::top_level(0, 3, 0);
    pos.path = vec![
        PathStep::Cell { cell: 0, block: 1 },
        PathStep::Cell { cell: 2, block: 0 },
    ];
    assert_eq!(selected_object(&cursor_at(pos)), SelectedObject::Table);
}
