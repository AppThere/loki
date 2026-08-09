// SPDX-License-Identifier: Apache-2.0

//! Keyboard input for the DOM reflow view (ADR-0017 sequencing step 12, first
//! slice of `TODO(dom-reflow-editing)`): character insert, Backspace,
//! Delete, Enter, and Ctrl shortcuts — dispatched through the exact same
//! handler functions `editor_keydown.rs` calls for the canvas path, so the
//! mutation behaviour (Loro edits, undo/redo stack, revision marks) is not a
//! second implementation of the same thing.
//!
//! # Arrow / Home / End navigation is deliberately not wired here
//!
//! `editor_keydown.rs`'s arrow-key branch resolves reflow navigation against
//! `ensure_reflow_layout` built at
//! `reflow_layout_content_width_pt(scroll_metrics.client_width)` — the
//! **viewport-responsive** width the canvas-reflow tile is painted at. This
//! view's column width ([`super::column_max_width_pt`]) is a fixed reading
//! measure, not viewport-derived; the DOM column still narrows on a small
//! window (ordinary CSS `max-width` constrained by its flex parent), but a
//! `ContinuousLayout` built from the Rust-side fixed width would not reflect
//! that narrower box. Reusing the arrow branch unmodified would silently
//! navigate against the wrong line geometry on exactly the windows where the
//! two widths disagree, which is worse than not navigating at all.
//! `TODO(dom-reflow-caret-nav)`: reconcile the two width sources (most likely
//! by measuring the column's real rendered width via `get_client_rect`,
//! which this view's ordinary DOM elements support and the canvas path's
//! elements do not — see `editing.rs`'s module docs) before wiring this up.

use dioxus::prelude::*;
use keyboard_types::Modifiers;

use super::editing::EditingCtx;
use crate::routes::editor::editor_keydown_backspace::handle_backspace_key;
use crate::routes::editor::editor_keydown_ctrl::{handle_ctrl_keys, handle_delete_key};
use crate::routes::editor::editor_keydown_enter::handle_enter_key;
use crate::routes::editor::editor_keydown_text::{
    SelectionRemoval, handle_character_key, remove_selection,
};

/// Builds the `onkeydown` closure for the DOM reflow view's column.
pub(super) fn make_dom_reflow_keydown_handler(ctx: EditingCtx) -> impl FnMut(Event<KeyboardData>) {
    move |evt: Event<KeyboardData>| {
        let key = evt.key();
        let modifiers = evt.modifiers();
        let doc_state = &ctx.doc_state;
        let cursor_state = ctx.cursor_state;
        let loro_doc = ctx.loro_doc;
        let undo_manager = ctx.undo_manager;
        let can_undo = ctx.can_undo;
        let can_redo = ctx.can_redo;
        let mut save_request = ctx.save_request;

        if modifiers.ctrl() || modifiers.meta() || modifiers.contains(Modifiers::SUPER) {
            if matches!(&key, Key::Character(c) if c.eq_ignore_ascii_case("s")) {
                let next = save_request.peek().wrapping_add(1);
                save_request.set(next);
                return;
            }
            handle_ctrl_keys(
                doc_state,
                cursor_state,
                loro_doc,
                undo_manager,
                can_undo,
                can_redo,
                modifiers,
                &key,
            );
            return;
        }

        let focus = cursor_state.read().focus.clone();
        let Some(focus) = focus else { return };

        match &key {
            Key::Character(ch) => {
                handle_character_key(
                    ch.clone(),
                    focus,
                    loro_doc,
                    doc_state,
                    cursor_state,
                    undo_manager,
                    can_undo,
                    can_redo,
                );
            }
            Key::Backspace => {
                handle_backspace_key(
                    focus,
                    loro_doc,
                    doc_state,
                    cursor_state,
                    undo_manager,
                    can_undo,
                    can_redo,
                );
            }
            Key::Delete => {
                if matches!(
                    remove_selection(
                        loro_doc,
                        doc_state,
                        cursor_state,
                        undo_manager,
                        can_undo,
                        can_redo,
                    ),
                    SelectionRemoval::NoSelection
                ) {
                    handle_delete_key(
                        focus,
                        loro_doc,
                        doc_state,
                        cursor_state,
                        undo_manager,
                        can_undo,
                        can_redo,
                    );
                }
            }
            Key::Enter => {
                handle_enter_key(
                    focus,
                    loro_doc,
                    doc_state,
                    cursor_state,
                    undo_manager,
                    can_undo,
                    can_redo,
                );
            }
            // Arrow/Home/End: TODO(dom-reflow-caret-nav) — see module docs.
            _ => {}
        }
    }
}
