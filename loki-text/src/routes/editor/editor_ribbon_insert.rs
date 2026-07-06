// SPDX-License-Identifier: Apache-2.0

//! Insert tab ribbon content for the document editor (Spec 04 M4).
//!
//! [`insert_tab_content`] returns the `Element` passed to
//! [`AtRibbon::tab_content`] when the Insert tab is active. It hosts the create
//! controls for objects with a native Loro mapping:
//!
//! - **Image** — picks a file, embeds it as a `data:` URI, and inserts an
//!   `Inline::Image` at the cursor (see [`super::editor_insert`]).
//! - **Table** — inserts an empty 2×2 table after the cursor's block; its cells
//!   are live editable containers (click in to edit).
//! - **Footnote** — inserts a footnote at the cursor with an empty, editable
//!   body.
//! - **Link** — opens the URL panel ([`super::editor_insert_panel`]).

use std::sync::{Arc, Mutex};

use appthere_ui::{
    AtIcon, AtRibbonGroup, AtRibbonIconButton, LUCIDE_FOOTNOTE, LUCIDE_IMAGE, LUCIDE_LINK,
    LUCIDE_TABLE,
};
use dioxus::prelude::*;
use loki_doc_model::MutationError;
use loki_i18n::fl;
use loro::LoroDoc;

use super::editor_insert::{insert_footnote_at_cursor, insert_table_after_cursor};
use super::editor_keydown_ctrl::post_mutation_sync;
use super::editor_keydown_text::set_collapsed_cursor;
use super::editor_ribbon_insert_image::spawn_pick_and_insert_image;
use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Live-document handles the Insert tab needs to create objects at the cursor.
#[derive(Clone)]
pub(super) struct InsertCtx {
    /// Document state for relayout after a mutation.
    pub doc_state: Arc<Mutex<DocumentState>>,
    /// The document's Loro CRDT handle.
    pub loro_doc: Signal<Option<loro::LoroDoc>>,
    /// Cursor state — the insertion point and dirty-tracking generation.
    pub cursor_state: Signal<CursorState>,
    /// Undo manager, refreshed after the mutation.
    pub undo_manager: Signal<Option<loro::UndoManager>>,
    /// Whether undo is available.
    pub can_undo: Signal<bool>,
    /// Whether redo is available.
    pub can_redo: Signal<bool>,
    /// Status-banner sink for success / error feedback.
    pub save_message: Signal<Option<String>>,
}

/// Builds the Insert tab ribbon content element.
///
/// `link_draft` is the shared hyperlink-panel signal (setting it to `Some("")`
/// opens the URL panel); `ctx` carries the handles the Image button needs to
/// pick a file and insert it at the cursor.
pub(super) fn insert_tab_content(
    mut link_draft: Signal<Option<String>>,
    ctx: InsertCtx,
) -> Element {
    rsx! {
        // ── Media group ───────────────────────────────────────────────────────
        AtRibbonGroup {
            label:      Some(fl!("ribbon-group-media")),
            aria_label: fl!("ribbon-group-media"),

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-insert-image-aria"),
                is_active:   false,
                is_disabled: false,
                on_click: {
                    let ctx = ctx.clone();
                    move |_| spawn_pick_and_insert_image(ctx.clone())
                },
                AtIcon { path_d: LUCIDE_IMAGE.to_string() }
            }
        }

        // ── Tables group ──────────────────────────────────────────────────────
        AtRibbonGroup {
            label:      Some(fl!("ribbon-group-tables")),
            aria_label: fl!("ribbon-group-tables"),

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-insert-table-aria"),
                is_active:   false,
                is_disabled: false,
                on_click: {
                    let ctx = ctx.clone();
                    move |_| run_insert(
                        &ctx,
                        // Move the caret into the new table's first cell.
                        |ldoc, cur| insert_table_after_cursor(ldoc, cur).map(|target| {
                            match target {
                                Some(pos) => InsertResult::Inserted(Some(pos)),
                                None => InsertResult::NoCursor,
                            }
                        }),
                        fl!("editor-insert-table-success"),
                    )
                },
                AtIcon { path_d: LUCIDE_TABLE.to_string() }
            }
        }

        // ── References group ──────────────────────────────────────────────────
        AtRibbonGroup {
            label:      Some(fl!("ribbon-group-references")),
            aria_label: fl!("ribbon-group-references"),

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-insert-footnote-aria"),
                is_active:   false,
                is_disabled: false,
                on_click: {
                    let ctx = ctx.clone();
                    // The footnote body is editable via its `BlockPath`, but the
                    // caret stays at the anchor (Word leaves it there too).
                    move |_| run_insert(
                        &ctx,
                        |ldoc, cur| insert_footnote_at_cursor(ldoc, cur).map(|inserted| {
                            if inserted {
                                InsertResult::Inserted(None)
                            } else {
                                InsertResult::NoCursor
                            }
                        }),
                        fl!("editor-insert-footnote-success"),
                    )
                },
                AtIcon { path_d: LUCIDE_FOOTNOTE.to_string() }
            }
        }

        // ── Links group ───────────────────────────────────────────────────────
        AtRibbonGroup {
            label:      Some(fl!("ribbon-group-links")),
            aria_label: fl!("ribbon-group-links"),

            AtRibbonIconButton {
                aria_label:  fl!("ribbon-insert-link-aria"),
                is_active:   link_draft.read().is_some(),
                is_disabled: false,
                on_click: move |_| {
                    if link_draft.read().is_some() {
                        link_draft.set(None);
                    } else {
                        link_draft.set(Some(String::new()));
                    }
                },
                AtIcon { path_d: LUCIDE_LINK.to_string() }
            }
        }
    }
}

/// What an Insert-tab `op` did with the live document.
pub(super) enum InsertResult {
    /// Content was inserted. If `Some`, the caret should collapse to this
    /// position after relayout (e.g. the new table's first cell); if `None`,
    /// the caret is left where it was.
    Inserted(Option<DocumentPosition>),
    /// There was no placed cursor, so nothing was inserted.
    NoCursor,
}

/// Runs a synchronous Insert-tab `op` against the live document, relays out,
/// syncs undo/redo, optionally moves the caret into the new object, and reports
/// the outcome through the status banner.
///
/// `op` returns [`InsertResult::Inserted`] (with an optional caret target) when
/// it mutated, [`InsertResult::NoCursor`] when there was no cursor, or `Err` on
/// failure — surfaced as the no-cursor / failure messages.
fn run_insert(
    ctx: &InsertCtx,
    op: impl FnOnce(&LoroDoc, &CursorState) -> Result<InsertResult, MutationError>,
    success: String,
) {
    let mut save_message = ctx.save_message;
    // `Ok(target)` — inserted, move caret to `target` if `Some`; `Err(true)` —
    // no cursor; `Err(false)` — the op failed.
    let outcome: Result<Option<DocumentPosition>, bool> = {
        let guard = ctx.loro_doc.read();
        let Some(ldoc) = guard.as_ref() else {
            return;
        };
        match op(ldoc, &ctx.cursor_state.read()) {
            Ok(InsertResult::Inserted(target)) => {
                apply_mutation_and_relayout(&ctx.doc_state, ldoc);
                Ok(target)
            }
            Ok(InsertResult::NoCursor) => Err(true),
            Err(_) => Err(false),
        }
    };
    match outcome {
        Ok(target) => {
            post_mutation_sync(
                &ctx.doc_state,
                ctx.loro_doc,
                ctx.cursor_state,
                ctx.undo_manager,
                ctx.can_undo,
                ctx.can_redo,
            );
            // After relayout, re-derive the caret's page from the fresh layout.
            if let Some(pos) = target {
                set_collapsed_cursor(&ctx.doc_state, ctx.cursor_state, pos);
            }
            save_message.set(Some(success));
        }
        Err(true) => save_message.set(Some(fl!("editor-insert-no-cursor"))),
        Err(false) => save_message.set(Some(fl!("editor-insert-failed"))),
    }
}
