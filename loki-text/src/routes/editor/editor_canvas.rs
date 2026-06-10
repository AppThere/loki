// SPDX-License-Identifier: Apache-2.0

//! Scrollable canvas area for the document editor.
//!
//! [`render_canvas_area`] returns the scroll container div that holds the
//! [`loki_renderer::DocumentView`] component.  It is a plain function
//! (not a Dioxus component) so it cannot call hooks; all reactive state is
//! received as copied signals from [`super::editor_inner::EditorInner`].
//!
//! # Blitz scroll mechanics
//!
//! `WindowEvent::MouseWheel` is handled in blitz-shell without routing through
//! Dioxus (`scroll_node_by_has_changed`, window.rs:388).  For scroll to engage,
//! `can_y_scroll` must be `true` (`overflow-y: auto` or `scroll`) and
//! `scroll_height() > 0`.  Using `flex: 1` left the container height indefinite
//! when Blitz failed to propagate the `height: 100vh` definite size through the
//! flex chain — taffy then expands the container to fit all children, making
//! `scroll_height() = 0`.  The scroll container therefore uses `flex: 1` within
//! a parent that has `height: 100vh` so taffy can resolve the fraction.
//!
//! Scroll position reaches Dioxus through the PATCH(loki) chain: blitz-dom
//! collects the nodes whose offsets changed (`scroll_node_by_collect`),
//! blitz-shell forwards them via `Document::handle_scroll_changes`, and
//! dioxus-native-dom dispatches `scroll` events with `NativeScrollData` —
//! the `onscroll` handler below receives them.
//!
//! TODO(partial-render): also feed scroll_offset into DocumentView's
//! ScrollState (appthere_canvas::on_scroll_event) so cache tiering tracks
//! the real viewport instead of assuming the top of the document.
//!
//! Click-to-cursor-position is handled by `make_mousedown_handler` in
//! `editor_pointer.rs`, which calls `hit_test_document` and updates the cursor.

use std::sync::Arc;

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::document::Document;
use loki_renderer::{DocumentView, RendererCursorPos};

use super::editor_error_view::EditorErrorView;
use super::editor_keydown::make_keydown_handler;
use super::editor_pointer::{
    make_mousemove_handler, make_touchend_handler, make_touchmove_handler,
};
use crate::editing::cursor::CursorState;
use crate::editing::hit_test::hit_test_page;
use crate::editing::state::DocumentState;
use crate::editing::touch::TouchInteractionState;
use crate::error::LoadError;
use loki_doc_model::loro_bridge::derive_loro_cursor;

/// Renders the scrollable canvas area for the document editor.
///
/// Plain function — no hooks allowed.  All reactive state is passed in as
/// copied signals.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_canvas_area(
    doc_state_mousedown: std::sync::Arc<std::sync::Mutex<DocumentState>>,
    doc_state_mousemove: std::sync::Arc<std::sync::Mutex<DocumentState>>,
    doc_state_touch: std::sync::Arc<std::sync::Mutex<DocumentState>>,
    doc_state_touchend: std::sync::Arc<std::sync::Mutex<DocumentState>>,
    doc_state_keydown: std::sync::Arc<std::sync::Mutex<DocumentState>>,
    doc_state_render: std::sync::Arc<std::sync::Mutex<DocumentState>>,
    doc_state_scroll: std::sync::Arc<std::sync::Mutex<DocumentState>>,
    mut is_dragging: Signal<bool>,
    mut drag_origin: Signal<Option<(f32, f32)>>,
    touch_state: Signal<Option<TouchInteractionState>>,
    window_width: Signal<f32>,
    mut scroll_offset: Signal<f32>,
    mut current_page: Signal<u32>,
    mut cursor_state: Signal<CursorState>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
    save_request: Signal<u32>,
    path_signal: Signal<String>,
    document_load: Resource<(String, Result<Document, LoadError>)>,
    mut canvas_hovered: Signal<bool>,
    page_gap_px: f32,
) -> Element {
    rsx! {
        div {
            // COMPAT(dioxus-native): flex: 1 is confirmed working. Requires
            // height: 100vh on the parent so Taffy can resolve the flex fraction.
            // tabindex="0" enables keyboard focus for onkeydown to fire.
            // autofocus ensures the canvas receives keyboard focus immediately
            // when the editor mounts, so the user can type without clicking first.
            // COMPAT(dioxus-native): scrollbar-width / scrollbar-color are
            // unconfirmed in Blitz — they are Stylo (Firefox CSS engine)
            // properties.  If unsupported the platform-default scrollbar is
            // shown; no functionality is lost.
            style: format!(
                "flex: 1; min-height: 0; overflow-y: auto; overflow-x: hidden; \
                 background: {bg}; padding: {p}px 0; \
                 scrollbar-width: thin; scrollbar-color: {thumb} transparent;",
                bg    = tokens::COLOR_SURFACE_BASE,
                p     = tokens::SPACE_6,
                thumb = if canvas_hovered() {
                    tokens::COLOR_SCROLLBAR_THUMB_HOVER
                } else {
                    tokens::COLOR_SCROLLBAR_THUMB
                },
            ),
            tabindex: "0",
            autofocus: "true",
            onmouseenter: move |_| { canvas_hovered.set(true); },
            onmouseleave: move |_| { canvas_hovered.set(false); },

            // Scroll events are dispatched by the patched Blitz shell
            // (PATCH(loki) in blitz-shell/blitz-dom/dioxus-native-dom) after a
            // wheel or touch gesture changes this container's scroll offset.
            // Updates the status-bar page indicator: the current page is the
            // one occupying the vertical centre of the viewport.
            onscroll: move |evt: ScrollEvent| {
                let top = evt.scroll_top() as f32;
                scroll_offset.set(top);
                let viewport_h = evt.client_height() as f32;
                let (page_h, count) = match doc_state_scroll.lock() {
                    Ok(s) => (s.page_height_px, s.page_count),
                    Err(_) => return,
                };
                let slot = page_h + page_gap_px;
                if slot <= 0.0 || count == 0 {
                    return;
                }
                let page = (((top + viewport_h * 0.5) / slot).floor() as i64 + 1)
                    .clamp(1, count as i64) as u32;
                if *current_page.peek() != page {
                    current_page.set(page);
                }
            },

            // Outer div records drag origin; cursor placement happens in
            // on_tile_click on the per-page div (element_coordinates, no origin math).
            onmousedown: move |evt: MouseEvent| {
                let c = evt.client_coordinates();
                drag_origin.set(Some((c.x as f32, c.y as f32)));
            },

            onmousemove: make_mousemove_handler(
                doc_state_mousemove,
                is_dragging,
                drag_origin,
                window_width,
                scroll_offset,
                cursor_state,
                page_gap_px,
            ),

            onmouseup: move |_| {
                is_dragging.set(false);
                drag_origin.set(None);
            },

            ontouchstart: move |evt: TouchEvent| {
                let touches = evt.touches();
                let Some(first) = touches.first() else { return };
                let c = first.client_coordinates();
                touch_state.clone().set(Some(TouchInteractionState::new(
                    0,
                    (c.x as f32, c.y as f32),
                )));
            },

            ontouchmove: make_touchmove_handler(
                doc_state_touch,
                touch_state,
                window_width,
                scroll_offset,
                loro_doc,
                cursor_state,
                page_gap_px,
            ),

            ontouchend: make_touchend_handler(
                doc_state_touchend,
                touch_state,
                window_width,
                scroll_offset,
                loro_doc,
                cursor_state,
                page_gap_px,
            ),

            onkeydown: make_keydown_handler(
                doc_state_keydown,
                cursor_state,
                loro_doc,
                undo_manager,
                can_undo,
                can_redo,
                save_request,
            ),

            match &*document_load.value().read_unchecked() {
                Some((loaded_path, Ok(doc))) if loaded_path == &path_signal() => {
                    // Use the live post-mutation document from doc_state when
                    // available; fall back to the original resource doc before
                    // seed_layout_from_document has run.
                    let arc_doc = doc_state_render
                        .lock()
                        .ok()
                        .and_then(|s| s.document.clone())
                        .unwrap_or_else(|| Arc::new(doc.clone()));
                    let cursor_pos = {
                        let cs = cursor_state.read();
                        cs.focus.as_ref().map(|pos| RendererCursorPos {
                            page_index: pos.page_index,
                            paragraph_index: pos.paragraph_index,
                            byte_offset: pos.byte_offset,
                        })
                    };
                    rsx! {
                        DocumentView {
                            doc: arc_doc,
                            // TODO(loki): measure actual viewport height — affects
                            // cache tier zones only, not visual correctness.
                            // See diagnostic report, finding 1.
                            viewport_height_px: 800.0,
                            cursor_pos,
                            on_tile_click: move |(page_index, x_pt, y_pt): (usize, f32, f32)| {
                                let layout_opt = {
                                    let Ok(state) = doc_state_mousedown.lock() else { return };
                                    state.paginated_layout.clone()
                                };
                                let Some(layout) = layout_opt else { return };
                                let Some(pos) = hit_test_page(page_index, x_pt, y_pt, &layout)
                                else {
                                    return;
                                };
                                let loro_cursor = loro_doc.read().as_ref().and_then(|ldoc| {
                                    derive_loro_cursor(ldoc, pos.paragraph_index, pos.byte_offset)
                                });
                                let mut cs = cursor_state.write();
                                cs.loro_cursor = loro_cursor;
                                cs.anchor = Some(pos.clone());
                                cs.focus = Some(pos);
                            },
                        }
                    }
                },

                Some((loaded_path, Err(e))) if loaded_path == &path_signal() => {
                    let msg = e.to_string();
                    rsx! { EditorErrorView { message: msg } }
                },

                _ => rsx! { div {} },
            }
        }
    }
}
