// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The document-edit batch a macro run produces (macro spec §6.2).
//!
//! Every `DocWrite` a macro performs is recorded as a [`DocEdit`] and collected
//! into a single [`EditBatch`]. The app applies the whole batch through the
//! editor's normal Loro mutation path as **one transaction**, so a macro run is
//! exactly **one undo entry** — a runaway-but-permitted macro is recoverable
//! with a single ⌘Z. Nothing here touches the CRDT directly; the batch is an
//! app-agnostic description of what to apply.

/// One document mutation requested by a macro (text object model, v1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocEdit {
    /// Replace the entire document body text.
    SetText(String),
    /// Append text to the end of the document body.
    AppendText(String),
}

/// The ordered edits a single macro run produced — applied atomically as one
/// undo entry (spec §6.2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditBatch {
    /// The edits, in the order the macro performed them.
    pub edits: Vec<DocEdit>,
}

impl EditBatch {
    /// An empty batch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the batch has no edits (the run made no document changes, so the
    /// app should create no undo entry).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// Number of edits in the batch.
    #[must_use]
    pub fn len(&self) -> usize {
        self.edits.len()
    }

    /// Folds the batch onto `text`, returning the resulting body — the same
    /// transformation the app applies to the live document, usable to preview or
    /// test the net effect.
    #[must_use]
    pub fn apply_to(&self, mut text: String) -> String {
        for edit in &self.edits {
            match edit {
                DocEdit::SetText(s) => text = s.clone(),
                DocEdit::AppendText(s) => text.push_str(s),
            }
        }
        text
    }
}
