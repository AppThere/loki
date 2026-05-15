// SPDX-License-Identifier: Apache-2.0

//! Keyboard event handler factory for the document canvas.

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use keyboard_types::Modifiers;
use loki_doc_model::loro_mutation::{delete_text, get_block_text, insert_text};
use loki_doc_model::{merge_block, split_block};

use super::editor_keydown_ctrl::{handle_ctrl_keys, sync_undo_state};

use crate::components::document_source::{DocumentState, apply_mutation_and_relayout};
use crate::editing::cursor::{
    CursorState, DocumentPosition, next_grapheme_boundary, prev_grapheme_boundary,
};
use crate::editing::navigation::{
    navigate_down, navigate_end, navigate_home, navigate_left, navigate_right, navigate_up,
};

// EditorMode removed — the editor is always in edit mode when a document is
// open. Distraction-free reading is handled by the View ribbon tab (future
// pass), not by a separate mode.

/// Builds the `on_keydown` closure for [`super::editor_inner::EditorInner`].
///
/// Dispatches printable characters, `Backspace`, `Delete`, arrow navigation,
/// `Home`/`End`, and `Enter` (paragraph split).  The returned closure is
/// passed to `WgpuSurface`'s `on_keydown` prop.
pub(super) fn make_keydown_handler(
    doc_state: Arc<Mutex<DocumentState>>,
    mut cursor_state: Signal<CursorState>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
) -> impl FnMut(Rc<KeyboardData>) {
    move |evt: Rc<KeyboardData>| {
        let key = evt.key();
        let modifiers = evt.modifiers();

        // NOTE: on macOS, Meta is the Cmd key. On Windows/Linux,
        // Ctrl is used for shortcuts — both are checked here for
        // cross-platform consistency. blitz-shell maps macOS
        // Cmd → Modifiers::SUPER (not META), so we check SUPER too.
        if modifiers.ctrl() || modifiers.meta() || modifiers.contains(Modifiers::SUPER) {
            handle_ctrl_keys(
                &doc_state,
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
            // ── Printable characters ──────────────────────────────────────────
            Key::Character(ch) => {
                let ch = ch.clone();
                {
                    let ldoc_guard = loro_doc.read();
                    let Some(ldoc) = ldoc_guard.as_ref() else {
                        return;
                    };
                    if insert_text(ldoc, focus.paragraph_index, focus.byte_offset, &ch).is_err() {
                        return;
                    }
                }
                {
                    let ldoc_guard = loro_doc.read();
                    let Some(ldoc) = ldoc_guard.as_ref() else {
                        return;
                    };
                    apply_mutation_and_relayout(&doc_state, ldoc);
                }
                sync_undo_state(undo_manager, can_undo, can_redo);
                let new_offset = focus.byte_offset + ch.len();
                let new_pos = DocumentPosition {
                    byte_offset: new_offset,
                    ..focus
                };
                let mut cs = cursor_state.write();
                cs.focus = Some(new_pos.clone());
                cs.anchor = Some(new_pos);
            }

            // ── Backspace ─────────────────────────────────────────────────────
            Key::Backspace => {
                if focus.byte_offset == 0 {
                    if focus.paragraph_index == 0 {
                        return;
                    }
                    let ldoc_guard = loro_doc.read();
                    let Some(ldoc) = ldoc_guard.as_ref() else {
                        return;
                    };
                    let Ok(merged_offset) = merge_block(ldoc, focus.paragraph_index) else {
                        return;
                    };
                    apply_mutation_and_relayout(&doc_state, ldoc);
                    sync_undo_state(undo_manager, can_undo, can_redo);
                    // TODO(3b-3): recompute page_index from layout after merge
                    let new_pos = DocumentPosition {
                        page_index: focus.page_index,
                        paragraph_index: focus.paragraph_index - 1,
                        byte_offset: merged_offset,
                    };
                    let mut cs = cursor_state.write();
                    cs.focus = Some(new_pos.clone());
                    cs.anchor = Some(new_pos);
                    return;
                }
                let text = {
                    let ldoc_guard = loro_doc.read();
                    ldoc_guard
                        .as_ref()
                        .map(|l| get_block_text(l, focus.paragraph_index))
                        .unwrap_or_default()
                };
                let prev = prev_grapheme_boundary(&text, focus.byte_offset);
                let len = focus.byte_offset - prev;
                {
                    let ldoc_guard = loro_doc.read();
                    let Some(ldoc) = ldoc_guard.as_ref() else {
                        return;
                    };
                    if delete_text(ldoc, focus.paragraph_index, prev, len).is_err() {
                        return;
                    }
                }
                {
                    let ldoc_guard = loro_doc.read();
                    let Some(ldoc) = ldoc_guard.as_ref() else {
                        return;
                    };
                    apply_mutation_and_relayout(&doc_state, ldoc);
                }
                sync_undo_state(undo_manager, can_undo, can_redo);
                let new_pos = DocumentPosition {
                    byte_offset: prev,
                    ..focus
                };
                let mut cs = cursor_state.write();
                cs.focus = Some(new_pos.clone());
                cs.anchor = Some(new_pos);
            }

            // ── Forward delete ────────────────────────────────────────────────
            Key::Delete => {
                let text = {
                    let ldoc_guard = loro_doc.read();
                    ldoc_guard
                        .as_ref()
                        .map(|l| get_block_text(l, focus.paragraph_index))
                        .unwrap_or_default()
                };
                if focus.byte_offset >= text.len() {
                    return;
                }
                let next = next_grapheme_boundary(&text, focus.byte_offset);
                let len = next - focus.byte_offset;
                {
                    let ldoc_guard = loro_doc.read();
                    let Some(ldoc) = ldoc_guard.as_ref() else {
                        return;
                    };
                    if delete_text(ldoc, focus.paragraph_index, focus.byte_offset, len).is_err() {
                        return;
                    }
                }
                {
                    let ldoc_guard = loro_doc.read();
                    let Some(ldoc) = ldoc_guard.as_ref() else {
                        return;
                    };
                    apply_mutation_and_relayout(&doc_state, ldoc);
                }
                sync_undo_state(undo_manager, can_undo, can_redo);
                // Cursor stays at the same offset after forward delete.
            }

            // ── Arrow-key navigation ──────────────────────────────────────────
            Key::ArrowLeft | Key::ArrowRight => {
                let shift_held = modifiers.shift();
                let layout_opt = {
                    let state = doc_state.lock().unwrap_or_else(|e| e.into_inner());
                    state.paginated_layout.clone()
                };
                let Some(layout) = layout_opt else { return };
                let ldoc_guard = loro_doc.read();
                let new_pos = if key == Key::ArrowLeft {
                    navigate_left(&focus, &layout, |idx| {
                        ldoc_guard
                            .as_ref()
                            .map(|l| get_block_text(l, idx))
                            .unwrap_or_default()
                    })
                } else {
                    navigate_right(&focus, &layout, |idx| {
                        ldoc_guard
                            .as_ref()
                            .map(|l| get_block_text(l, idx))
                            .unwrap_or_default()
                    })
                };
                if let Some(np) = new_pos {
                    let mut cs = cursor_state.write();
                    cs.focus = Some(np.clone());
                    if !shift_held {
                        cs.anchor = Some(np);
                    }
                }
            }

            Key::ArrowUp | Key::ArrowDown => {
                let shift_held = modifiers.shift();
                let layout_opt = {
                    let state = doc_state.lock().unwrap_or_else(|e| e.into_inner());
                    state.paginated_layout.clone()
                };
                let Some(layout) = layout_opt else { return };
                let new_pos = if key == Key::ArrowUp {
                    navigate_up(&focus, &layout)
                } else {
                    navigate_down(&focus, &layout)
                };
                if let Some(np) = new_pos {
                    let mut cs = cursor_state.write();
                    cs.focus = Some(np.clone());
                    if !shift_held {
                        cs.anchor = Some(np);
                    }
                }
            }

            Key::Home | Key::End => {
                let shift_held = modifiers.shift();
                let layout_opt = {
                    let state = doc_state.lock().unwrap_or_else(|e| e.into_inner());
                    state.paginated_layout.clone()
                };
                let Some(layout) = layout_opt else { return };
                let ldoc_guard = loro_doc.read();
                let new_pos = if key == Key::Home {
                    navigate_home(&focus, &layout)
                } else {
                    navigate_end(&focus, &layout, |idx| {
                        ldoc_guard
                            .as_ref()
                            .map(|l| get_block_text(l, idx))
                            .unwrap_or_default()
                    })
                };
                if let Some(np) = new_pos {
                    let mut cs = cursor_state.write();
                    cs.focus = Some(np.clone());
                    if !shift_held {
                        cs.anchor = Some(np);
                    }
                }
            }

            // ── Enter — split paragraph ───────────────────────────────────────
            Key::Enter => {
                let ldoc_guard = loro_doc.read();
                let Some(ldoc) = ldoc_guard.as_ref() else {
                    return;
                };
                if split_block(ldoc, focus.paragraph_index, focus.byte_offset).is_err() {
                    return;
                }
                apply_mutation_and_relayout(&doc_state, ldoc);
                sync_undo_state(undo_manager, can_undo, can_redo);
                // TODO(3b-3): recompute page_index from layout after split
                let new_pos = DocumentPosition {
                    page_index: focus.page_index,
                    paragraph_index: focus.paragraph_index + 1,
                    byte_offset: 0,
                };
                let mut cs = cursor_state.write();
                cs.focus = Some(new_pos.clone());
                cs.anchor = Some(new_pos);
            }

            _ => {}
        }
    }
}
