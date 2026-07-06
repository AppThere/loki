// SPDX-License-Identifier: Apache-2.0

//! Table **contextual** ribbon tab (Spec 04 M5, plan 4a.2).
//!
//! [`table_tab_content`] is rendered only while the caret is inside a table (the
//! `selected_object` signal is `Table`). It offers table-scoped operations; the
//! first is **Delete Table**, which removes the whole table block the caret sits
//! in. Row/column operations are future work (they need structural CRDT table
//! mutations).

use std::sync::{Arc, Mutex};

use appthere_ui::{
    AT_TABLE_COL_DELETE, AT_TABLE_COL_INSERT, AT_TABLE_ROW_DELETE, AT_TABLE_ROW_INSERT, AtIcon,
    AtRibbonGroup, AtRibbonIconButton, LUCIDE_TRASH_2, RibbonTabDesc,
};
use dioxus::prelude::*;
use loki_doc_model::{delete_block, table_grid_dims};
use loki_i18n::fl;

use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_keydown_text::set_collapsed_cursor;
use super::editor_ribbon_table_ops::{TableOp, run_table_op};
use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::editing::selected_object::{SelectedObject, selected_object};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Index of the Table contextual tab in the ribbon strip — it follows the four
/// core tabs (Write=0, Insert=1, Layout=2, Publish=3), so any `active_tab >= 4`
/// is the contextual tab.
const CONTEXTUAL_TAB_INDEX: usize = 4;

/// The ribbon tab descriptors for the current `selected` object: the four core
/// tabs, plus the Table contextual tab (amber) when the caret is in a table.
///
/// Pure — the appearance logic is unit-tested without a Dioxus runtime.
pub(super) fn ribbon_tabs(selected: SelectedObject) -> Vec<RibbonTabDesc> {
    let mut tabs = vec![
        RibbonTabDesc {
            label: fl!("ribbon-tab-write"),
            is_contextual: false,
            aria_label: None,
        },
        RibbonTabDesc {
            label: fl!("ribbon-tab-insert"),
            is_contextual: false,
            aria_label: None,
        },
        RibbonTabDesc {
            label: fl!("ribbon-tab-layout"),
            is_contextual: false,
            aria_label: None,
        },
        RibbonTabDesc {
            label: fl!("ribbon-tab-publish"),
            is_contextual: false,
            aria_label: None,
        },
    ];
    if selected == SelectedObject::Table {
        tabs.push(RibbonTabDesc {
            label: fl!("ribbon-tab-table"),
            is_contextual: true,
            aria_label: None,
        });
    }
    tabs
}

/// Derives the contextual-tab state and returns `(tabs, table_selected)` for the
/// ribbon (Spec 04 M5, plan 4a.2).
///
/// Also wires the fallback effect: when the caret leaves the table while its
/// contextual tab is active, the active tab resets to the first (Write) tab so
/// the ribbon never shows an orphaned contextual selection. Called once,
/// unconditionally, from `EditorInner`.
pub(super) fn use_ribbon_tabs(
    cursor_state: Signal<CursorState>,
    mut active_ribbon_tab: Signal<usize>,
) -> (Vec<RibbonTabDesc>, bool) {
    let selected = use_memo(move || selected_object(&cursor_state.read()));
    use_effect(move || {
        if selected() == SelectedObject::None && active_ribbon_tab() >= CONTEXTUAL_TAB_INDEX {
            active_ribbon_tab.set(0);
        }
    });
    let sel = selected();
    (ribbon_tabs(sel), sel == SelectedObject::Table)
}

/// The document's total top-level block count across all sections, or `0` when
/// no document is loaded.
fn block_count(doc_state: &Arc<Mutex<DocumentState>>) -> usize {
    doc_state
        .lock()
        .ok()
        .and_then(|s| {
            s.document
                .as_ref()
                .map(|d| d.sections.iter().map(|sec| sec.blocks.len()).sum())
        })
        .unwrap_or(0)
}

/// The `(rows, cols)` of the simple-grid table the caret is in, else `None`
/// (no caret, not in a table, or a non-simple-grid table where structural
/// row/column ops are unsupported).
fn table_dims_at_caret(
    loro_doc: Signal<Option<loro::LoroDoc>>,
    cursor_state: Signal<CursorState>,
) -> Option<(usize, usize)> {
    let idx = cursor_state.peek().focus.as_ref()?.paragraph_index;
    let guard = loro_doc.read();
    table_grid_dims(guard.as_ref()?, idx)
}

/// Builds the Table contextual tab content.
pub(super) fn table_tab_content(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) -> Element {
    let ds = Arc::clone(doc_state);
    // One Arc clone per row/column button — each on_click closure borrows its own.
    let ds_row_ins = Arc::clone(doc_state);
    let ds_row_del = Arc::clone(doc_state);
    let ds_col_ins = Arc::clone(doc_state);
    let ds_col_del = Arc::clone(doc_state);
    // Never delete the document's only block — that would leave nothing to edit.
    let only_block = block_count(doc_state) <= 1;
    // Row/column ops need a simple grid; delete is bounded to keep ≥1 row/col.
    let dims = table_dims_at_caret(loro_doc, cursor_state);
    let simple = dims.is_some();
    let (rows, cols) = dims.unwrap_or((0, 0));

    rsx! {
        // ── Rows & Columns group ──────────────────────────────────────────────
        AtRibbonGroup {
            label:      Some(fl!("ribbon-group-table-rows")),
            aria_label: fl!("ribbon-group-table-rows"),

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-table-row-insert-aria"),
                is_active:   false,
                is_disabled: !simple,
                on_click: move |_| run_table_op(
                    TableOp::InsertRow, &ds_row_ins, loro_doc, cursor_state,
                    undo_manager, can_undo, can_redo,
                ),
                AtIcon { path_d: AT_TABLE_ROW_INSERT.to_string() }
            }
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-table-row-delete-aria"),
                is_active:   false,
                is_disabled: !simple || rows <= 1,
                on_click: move |_| run_table_op(
                    TableOp::DeleteRow, &ds_row_del, loro_doc, cursor_state,
                    undo_manager, can_undo, can_redo,
                ),
                AtIcon { path_d: AT_TABLE_ROW_DELETE.to_string() }
            }
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-table-col-insert-aria"),
                is_active:   false,
                is_disabled: !simple,
                on_click: move |_| run_table_op(
                    TableOp::InsertColumn, &ds_col_ins, loro_doc, cursor_state,
                    undo_manager, can_undo, can_redo,
                ),
                AtIcon { path_d: AT_TABLE_COL_INSERT.to_string() }
            }
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-table-col-delete-aria"),
                is_active:   false,
                is_disabled: !simple || cols <= 1,
                on_click: move |_| run_table_op(
                    TableOp::DeleteColumn, &ds_col_del, loro_doc, cursor_state,
                    undo_manager, can_undo, can_redo,
                ),
                AtIcon { path_d: AT_TABLE_COL_DELETE.to_string() }
            }
        }

        // ── Table group ───────────────────────────────────────────────────────
        AtRibbonGroup {
            label:      Some(fl!("ribbon-group-table")),
            aria_label: fl!("ribbon-group-table"),

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-table-delete-aria"),
                is_active:   false,
                is_disabled: only_block,
                on_click: move |_| {
                    delete_current_table(
                        &ds, loro_doc, cursor_state, undo_manager, can_undo, can_redo,
                    );
                },
                AtIcon { path_d: LUCIDE_TRASH_2.to_string() }
            }
        }
    }
}

/// Deletes the table the caret is inside (its root block) and re-homes the
/// caret to the block that takes its place (or the previous block if it was
/// last), at offset 0.
fn delete_current_table(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    cursor_state: Signal<CursorState>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) {
    // The table is the caret's root block (works from inside any of its cells).
    let Some(root) = cursor_state
        .peek()
        .focus
        .as_ref()
        .map(|f| f.paragraph_index)
    else {
        return;
    };
    // Re-check the guard at click time (the document may have changed).
    if block_count(doc_state) <= 1 {
        return;
    }
    {
        let guard = loro_doc.read();
        let Some(ldoc) = guard.as_ref() else {
            return;
        };
        if delete_block(ldoc, root).is_err() {
            return;
        }
        apply_mutation_and_relayout(doc_state, ldoc);
    }
    post_mutation_sync(
        doc_state,
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
    );
    // `remaining >= 1` (guarded above), so this index is valid; the page is
    // re-derived from the fresh layout.
    let remaining = block_count(doc_state);
    let target = root.min(remaining.saturating_sub(1));
    set_collapsed_cursor(
        doc_state,
        cursor_state,
        DocumentPosition::top_level(0, target, 0),
    );
}

#[cfg(test)]
#[path = "editor_ribbon_table_tests.rs"]
mod tests;
