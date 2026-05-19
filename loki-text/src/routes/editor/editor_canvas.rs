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
//! `node.scroll_offset` is internal to blitz-dom (no public Dioxus API), so
//! `visible_rect` stays `None` and `onwheel` handlers never fire.
//!
//! TODO(partial-render): wire scroll_offset → DocumentView viewport once Blitz
//! exposes a scroll-position hook to Dioxus components.
//!
//! TODO(cursor-click): restore click-to-cursor-position via hit_test_document
//! once a suitable DocumentPosition → DocumentView mapping is established.

use std::sync::Arc;

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::document::Document;
use loki_renderer::DocumentView;

use super::editor_error_view::EditorErrorView;
use super::editor_keydown::make_keydown_handler;
use super::editor_pointer::{
    make_mousemove_handler, make_touchend_handler, make_touchmove_handler,
};
use crate::editing::cursor::CursorState;
use crate::editing::state::DocumentState;
use crate::editing::touch::TouchInteractionState;
use crate::error::LoadError;

/// Renders the scrollable canvas area for the document editor.
///
/// Plain function — no hooks allowed.  All reactive state is passed in as
/// copied signals.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_canvas_area(
    doc_state_mousemove: std::sync::Arc<std::sync::Mutex<DocumentState>>,
    doc_state_touch: std::sync::Arc<std::sync::Mutex<DocumentState>>,
    doc_state_touchend: std::sync::Arc<std::sync::Mutex<DocumentState>>,
    doc_state_keydown: std::sync::Arc<std::sync::Mutex<DocumentState>>,
    mut is_dragging: Signal<bool>,
    mut drag_origin: Signal<Option<(f32, f32)>>,
    touch_state: Signal<Option<TouchInteractionState>>,
    window_width: Signal<f32>,
    scroll_offset: Signal<f32>,
    cursor_state: Signal<CursorState>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
    path_signal: Signal<String>,
    document_load: Resource<(String, Result<Document, LoadError>)>,
    page_gap_px: f32,
) -> Element {
    rsx! {
        div {
            // COMPAT(dioxus-native): flex: 1 is confirmed working. Requires
            // height: 100vh on the parent so Taffy can resolve the flex fraction.
            // tabindex="0" enables keyboard focus for onkeydown to fire.
            style: format!(
                "flex: 1; min-height: 0; overflow-y: auto; overflow-x: hidden; \
                 background: {bg}; padding: {p}px 0;",
                bg = tokens::COLOR_SURFACE_BASE,
                p  = tokens::SPACE_6,
            ),
            tabindex: "0",

            onmousedown: move |evt| {
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
            ),

            match &*document_load.value().read_unchecked() {
                Some((loaded_path, Ok(doc))) if loaded_path == &path_signal() => {
                    let arc_doc = Arc::new(doc.clone());
                    rsx! {
                        DocumentView {
                            doc: arc_doc,
                            viewport_height_px: 800.0,
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
