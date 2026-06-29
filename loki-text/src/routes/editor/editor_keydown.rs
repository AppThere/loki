// SPDX-License-Identifier: Apache-2.0

//! Keyboard event handler factory for the document canvas.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use keyboard_types::Modifiers;
use loki_doc_model::loro_mutation::{delete_text, get_block_text, insert_text};
use loki_doc_model::merge_block;

use loki_renderer::ViewMode;
use loki_renderer::render_layout::reflow_content_width_pt;

use super::editor_keydown_ctrl::{
    handle_ctrl_keys, handle_delete_key, handle_enter_key, post_mutation_sync,
};
use super::editor_scrollbar::ScrollMetrics;

use crate::editing::cursor::{CursorState, DocumentPosition, prev_grapheme_boundary};
use crate::editing::navigation::{
    navigate_down, navigate_end, navigate_home, navigate_left, navigate_right, navigate_up,
};
use crate::editing::reflow_nav::{
    reflow_navigate_down, reflow_navigate_end, reflow_navigate_home, reflow_navigate_left,
    reflow_navigate_right, reflow_navigate_up,
};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout, ensure_reflow_layout};

// EditorMode removed — the editor is always in edit mode when a document is
// open. Distraction-free reading is handled by the View ribbon tab (future
// pass), not by a separate mode.

/// Builds the `onkeydown` closure for the document canvas scroll container.
///
/// Dispatches printable characters, `Backspace`, `Delete`, arrow navigation,
/// `Home`/`End`, and `Enter` (paragraph split).
#[allow(clippy::too_many_arguments)]
pub(super) fn make_keydown_handler(
    doc_state: Arc<Mutex<DocumentState>>,
    mut cursor_state: Signal<CursorState>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
    mut save_request: Signal<u32>,
    view_mode: Signal<ViewMode>,
    scroll_metrics: Signal<ScrollMetrics>,
) -> impl FnMut(Event<KeyboardData>) {
    move |evt: Event<KeyboardData>| {
        let key = evt.key();
        let modifiers = evt.modifiers();

        // NOTE: on macOS, Meta is the Cmd key. On Windows/Linux,
        // Ctrl is used for shortcuts — both are checked here for
        // cross-platform consistency. blitz-shell maps macOS
        // Cmd → Modifiers::SUPER (not META), so we check SUPER too.
        if modifiers.ctrl() || modifiers.meta() || modifiers.contains(Modifiers::SUPER) {
            // Ctrl/Cmd+S → request a save. EditorInner performs it (it owns the
            // tab/recents context the keydown handler can't reach).
            if matches!(&key, Key::Character(c) if c.eq_ignore_ascii_case("s")) {
                let next = save_request.peek().wrapping_add(1);
                save_request.set(next);
                return;
            }
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
                post_mutation_sync(
                    &doc_state,
                    loro_doc,
                    cursor_state,
                    undo_manager,
                    can_undo,
                    can_redo,
                );
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
                    post_mutation_sync(
                        &doc_state,
                        loro_doc,
                        cursor_state,
                        undo_manager,
                        can_undo,
                        can_redo,
                    );
                    // TODO(3b-3): recompute page_index from layout after merge
                    let new_pos = DocumentPosition {
                        page_index: focus.page_index,
                        paragraph_index: focus.paragraph_index - 1,
                        byte_offset: merged_offset,
                        // Same container as the merged-from paragraph.
                        path: focus.path.clone(),
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
                post_mutation_sync(
                    &doc_state,
                    loro_doc,
                    cursor_state,
                    undo_manager,
                    can_undo,
                    can_redo,
                );
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
                handle_delete_key(
                    focus,
                    loro_doc,
                    &doc_state,
                    cursor_state,
                    undo_manager,
                    can_undo,
                    can_redo,
                );
            }

            // ── Arrow / Home / End navigation (mode-aware) ────────────────────
            // Shift extends the selection (keeps the anchor); no Shift collapses
            // it to the new focus. Reflow uses the reflowed line geometry; the
            // paginated path is unchanged.
            Key::ArrowLeft
            | Key::ArrowRight
            | Key::ArrowUp
            | Key::ArrowDown
            | Key::Home
            | Key::End => {
                let shift_held = modifiers.shift();
                let ldoc_guard = loro_doc.read();
                let get_text = |idx: usize| {
                    ldoc_guard
                        .as_ref()
                        .map(|l| get_block_text(l, idx))
                        .unwrap_or_default()
                };

                let new_pos = if view_mode() == ViewMode::Reflow {
                    let width_px = scroll_metrics.peek().client_width;
                    if width_px <= 1.0 {
                        return;
                    }
                    let content_w = reflow_content_width_pt(width_px);
                    let Some(layout) = ensure_reflow_layout(&doc_state, content_w) else {
                        return;
                    };
                    match &key {
                        Key::ArrowLeft => reflow_navigate_left(&focus, &layout, get_text),
                        Key::ArrowRight => reflow_navigate_right(&focus, &layout, get_text),
                        Key::ArrowUp => reflow_navigate_up(&focus, &layout),
                        Key::ArrowDown => reflow_navigate_down(&focus, &layout),
                        Key::Home => reflow_navigate_home(&focus, &layout),
                        Key::End => reflow_navigate_end(&focus, &layout, get_text),
                        _ => None,
                    }
                } else {
                    let layout_opt = {
                        let state = doc_state.lock().unwrap_or_else(|e| e.into_inner());
                        state.paginated_layout.clone()
                    };
                    let Some(layout) = layout_opt else { return };
                    match &key {
                        Key::ArrowLeft => navigate_left(&focus, &layout, get_text),
                        Key::ArrowRight => navigate_right(&focus, &layout, get_text),
                        Key::ArrowUp => navigate_up(&focus, &layout),
                        Key::ArrowDown => navigate_down(&focus, &layout),
                        Key::Home => navigate_home(&focus, &layout),
                        Key::End => navigate_end(&focus, &layout, get_text),
                        _ => None,
                    }
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
                handle_enter_key(
                    focus,
                    loro_doc,
                    &doc_state,
                    cursor_state,
                    undo_manager,
                    can_undo,
                    can_redo,
                );
            }

            _ => {}
        }
    }
}
