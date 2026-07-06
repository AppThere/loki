// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The kind of document object the caret is currently on — the signal that
//! drives the ribbon's **contextual tabs** (Spec 04 M5, plan 4a.2).
//!
//! Word/LibreOffice show an object-specific tab (Table Tools, Picture Tools, …)
//! only while the relevant object is selected. This module derives that state,
//! purely, from the cursor so the editor can add/remove the contextual tab.

use loki_doc_model::PathStep;

use crate::editing::cursor::CursorState;

/// What the caret/selection is currently inside, for contextual-tab display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectedObject {
    /// An ordinary top-level paragraph — no contextual tab.
    #[default]
    None,
    /// Inside a table cell — show the **Table** contextual tab.
    Table,
}

/// Derives the [`SelectedObject`] from the cursor's focus position.
///
/// A focus whose path descends through a table cell ([`PathStep::Cell`]) is a
/// table selection; anything else — a plain paragraph, or a note body — is
/// [`SelectedObject::None`] for now (note/image contextual tabs are future
/// work). Nested tables report `Table` from the outermost cell down, which is
/// what the caret is visibly inside.
#[must_use]
pub fn selected_object(cursor: &CursorState) -> SelectedObject {
    let Some(focus) = cursor.focus.as_ref() else {
        return SelectedObject::None;
    };
    if focus
        .path
        .iter()
        .any(|step| matches!(step, PathStep::Cell { .. }))
    {
        SelectedObject::Table
    } else {
        SelectedObject::None
    }
}

#[cfg(test)]
#[path = "selected_object_tests.rs"]
mod tests;
