// SPDX-License-Identifier: Apache-2.0

//! `use_signal` initialisers and payload aliases for the draft signals that
//! `editor_inner` creates but must not name the types of (they are private to
//! this module — `editor_inner` only threads the signals through).

use super::draft_table::TableStyleDraft;
use super::list_form_draft::ListLevelDraft;

/// `use_signal` initialiser for the table draft.
pub(in crate::routes::editor) fn table_draft_none() -> Option<TableStyleDraft> {
    None
}

/// The table-draft signal's payload.
pub(in crate::routes::editor) type TableStyleDraftHandle = TableStyleDraft;

/// `use_signal` initialiser for the list-level draft (§10 tier 5).
pub(in crate::routes::editor) fn list_level_draft_none() -> Option<ListLevelDraft> {
    None
}

/// The list-level-draft signal's payload.
pub(in crate::routes::editor) type ListLevelDraftHandle = ListLevelDraft;
