// SPDX-License-Identifier: Apache-2.0

//! Keyboard event handler factory for the document canvas.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use keyboard_types::Modifiers;
use loki_doc_model::loro_mutation::{get_block_text, get_block_text_at};

use loki_renderer::ViewMode;
use loki_renderer::render_layout::reflow_layout_content_width_pt;

use super::editor_keydown_backspace::handle_backspace_key;
use super::editor_keydown_ctrl::{handle_ctrl_keys, handle_delete_key, post_mutation_sync};
use super::editor_keydown_newline::{enter_or_line_break, insert_text_or_paragraph_breaks};
use super::editor_keydown_text::{SelectionRemoval, remove_selection};
use super::editor_scrollbar::ScrollMetrics;

use crate::editing::cursor::CursorState;
use crate::editing::navigation::{
    navigate_down, navigate_end, navigate_home, navigate_left, navigate_right, navigate_up,
};
use crate::editing::reflow_nav::{
    reflow_navigate_down, reflow_navigate_end, reflow_navigate_home, reflow_navigate_left,
    reflow_navigate_right, reflow_navigate_up,
};
use crate::editing::state::{DocumentState, ensure_reflow_layout};

// EditorMode removed — the editor is always in edit mode when a document is
// open. Distraction-free reading is handled by the View ribbon tab (future
// pass), not by a separate mode.

/// Document-level shortcut callbacks the canvas keydown handler can't build
/// itself (they need the tab/recents context): Ctrl+N and Ctrl+O.
#[derive(Clone, Copy)]
pub(super) struct DocShortcuts {
    /// New blank document (the Document group's main New action).
    pub on_new: Callback<String>,
    /// The platform open picker.
    pub on_open: Callback<()>,
    /// Ctrl/Cmd+P — the Print dialog (§6).
    pub on_print: Callback<()>,
}

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
    shortcuts: DocShortcuts,
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
            // Ctrl/Cmd+N / +O — the Document group's New and Open (§4).
            if matches!(&key, Key::Character(c) if c.eq_ignore_ascii_case("n")) {
                shortcuts
                    .on_new
                    .call(super::editor_document_actions::BLANK_MENU_ID.to_string());
                return;
            }
            if matches!(&key, Key::Character(c) if c.eq_ignore_ascii_case("o")) {
                shortcuts.on_open.call(());
                return;
            }
            if matches!(&key, Key::Character(c) if c.eq_ignore_ascii_case("p")) {
                shortcuts.on_print.call(());
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
            // ── Printable characters (replace the selection if one is active) ─
            // A soft keyboard's Enter arrives *here*, as `Character("\n")` —
            // see `editor_keydown_newline`, which splits paragraphs on it.
            Key::Character(ch) => {
                insert_text_or_paragraph_breaks(
                    ch.clone(),
                    focus,
                    loro_doc,
                    &doc_state,
                    cursor_state,
                    undo_manager,
                    can_undo,
                    can_redo,
                );
            }

            // ── Backspace (selection removal / block merge / grapheme) ────────
            Key::Backspace => {
                handle_backspace_key(
                    focus,
                    loro_doc,
                    &doc_state,
                    cursor_state,
                    undo_manager,
                    can_undo,
                    can_redo,
                );
            }

            // ── Forward delete ────────────────────────────────────────────────
            Key::Delete => {
                // An active selection is removed instead of the next grapheme;
                // a rejected (cross-container) selection swallows the key.
                if matches!(
                    remove_selection(
                        loro_doc,
                        &doc_state,
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
                        &doc_state,
                        cursor_state,
                        undo_manager,
                        can_undo,
                        can_redo,
                    );
                }
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
                // Reflow navigation addresses top-level blocks by flat index;
                // the paginated path is path-aware so navigation works inside
                // table cells and note bodies too (4b.4).
                let get_text = |idx: usize| {
                    ldoc_guard
                        .as_ref()
                        .map(|l| get_block_text(l, idx))
                        .unwrap_or_default()
                };
                let get_text_at = |bp: &loki_doc_model::BlockPath| {
                    ldoc_guard
                        .as_ref()
                        .map(|l| get_block_text_at(l, bp))
                        .unwrap_or_default()
                };

                let new_pos = if view_mode() == ViewMode::Reflow {
                    let width_px = scroll_metrics.peek().client_width;
                    if width_px <= 1.0 {
                        return;
                    }
                    let content_w = reflow_layout_content_width_pt(width_px);
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
                        Key::ArrowLeft => navigate_left(&focus, &layout, get_text_at),
                        Key::ArrowRight => navigate_right(&focus, &layout, get_text_at),
                        Key::ArrowUp => navigate_up(&focus, &layout),
                        Key::ArrowDown => navigate_down(&focus, &layout),
                        Key::Home => navigate_home(&focus, &layout),
                        Key::End => navigate_end(&focus, &layout, get_text_at),
                        _ => None,
                    }
                    // A move inside a page-spanning paragraph can land on a
                    // line shown on a different page — re-derive the page.
                    .map(|np| crate::editing::page_locate::recompute_page_index(&layout, &np))
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
            // ── Tab / Shift-Tab: list item demote / promote (§10 tier 3) ─────
            Key::Tab => {
                // The event driver runs this handler before the blitz default
                // action, so preventing the default is what keeps focus in the
                // canvas. A non-list paragraph (or a level already at its
                // boundary) changes nothing and falls through to the ordinary
                // focus traversal — Tab must not invent a list.
                let changed = {
                    let ldoc_guard = loro_doc.read();
                    match ldoc_guard.as_ref() {
                        Some(ldoc) => {
                            let delta: i8 = if modifiers.shift() { -1 } else { 1 };
                            let changed = super::editor_lists::change_list_level(
                                ldoc,
                                &cursor_state.read(),
                                delta,
                            );
                            if changed {
                                crate::editing::state::apply_mutation_and_relayout(
                                    &doc_state, ldoc,
                                );
                            }
                            changed
                        }
                        None => false,
                    }
                };
                if changed {
                    evt.prevent_default();
                    post_mutation_sync(
                        &doc_state,
                        loro_doc,
                        cursor_state,
                        undo_manager,
                        can_undo,
                        can_redo,
                    );
                }
            }

            Key::Enter => {
                enter_or_line_break(
                    modifiers.shift(),
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
