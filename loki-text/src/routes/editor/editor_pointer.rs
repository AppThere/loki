// SPDX-License-Identifier: Apache-2.0

//! Mouse and touch event handler factories for the document canvas.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::loro_bridge::derive_loro_cursor;
use loki_doc_model::loro_mutation::get_block_text;

use loki_renderer::ViewMode;

use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::editing::hit_test::hit_test_document;
use crate::editing::state::DocumentState;
use crate::editing::touch::{TouchInteractionState, TouchPhase, word_boundaries_at};

// EditorMode removed — the editor is always in edit mode when a document is
// open. Distraction-free reading is handled by the View ribbon tab (future
// pass), not by a separate mode.

/// Builds the `onmousemove` handler for drag-selection.
///
/// Guards behind a 4 CSS-px drag threshold to prevent cursor jitter during
/// a plain click from creating a spurious selection.  Transitions
/// `is_dragging` to `true` the first time the threshold is exceeded.
#[allow(clippy::too_many_arguments)]
pub(super) fn make_mousemove_handler(
    doc_state: Arc<Mutex<DocumentState>>,
    mut is_dragging: Signal<bool>,
    drag_origin: Signal<Option<(f32, f32)>>,
    window_width: Signal<f32>,
    scroll_offset: Signal<f32>,
    mut cursor_state: Signal<CursorState>,
    page_gap_px: f32,
    view_mode: Signal<ViewMode>,
) -> impl FnMut(MouseEvent) {
    move |evt: MouseEvent| {
        // Reflow drag-select is handled per-tile (clean tile-local coordinates);
        // this window-relative paginated path must not interfere there.
        if view_mode() == ViewMode::Reflow {
            return;
        }
        if drag_origin().is_none() {
            return;
        }
        const DRAG_THRESHOLD_SQ: f32 = 4.0 * 4.0; // 4 CSS px
        let coords = evt.client_coordinates();
        let cx = coords.x as f32;
        let cy = coords.y as f32;
        if !is_dragging() {
            if let Some((ox, oy)) = drag_origin() {
                let dx = cx - ox;
                let dy = cy - oy;
                if dx * dx + dy * dy < DRAG_THRESHOLD_SQ {
                    return;
                }
            }
            is_dragging.set(true);
        }
        let (layout_opt, page_width_px, page_height_px) = {
            let Ok(state) = doc_state.lock() else { return };
            (
                state.paginated_layout.clone(),
                state.page_width_px,
                state.page_height_px,
            )
        };
        let Some(layout) = layout_opt else { return };
        let x_off = (window_width() - page_width_px).max(0.0) / 2.0;
        let origin = (x_off, tokens::TOOLBAR_HEIGHT_TOP + tokens::SPACE_6);
        if let Some(p) = hit_test_document(
            cx,
            cy,
            origin,
            scroll_offset(),
            &layout,
            page_width_px,
            page_height_px,
            page_gap_px,
        ) {
            cursor_state.write().focus = Some(p);
        }
    }
}

/// Builds the `ontouchmove` handler for touch drag and long-press word selection.
pub(super) fn make_touchmove_handler(
    doc_state: Arc<Mutex<DocumentState>>,
    mut touch_state: Signal<Option<TouchInteractionState>>,
    window_width: Signal<f32>,
    scroll_offset: Signal<f32>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    mut cursor_state: Signal<CursorState>,
    page_gap_px: f32,
) -> impl FnMut(TouchEvent) {
    move |evt: TouchEvent| {
        let Some(mut ts) = touch_state() else { return };
        let touches = evt.touches();
        let Some(first) = touches.first() else { return };
        let c = first.client_coordinates();
        let new_pos = (c.x as f32, c.y as f32);
        let became_scroll = ts.update_move(new_pos);
        if became_scroll {
            if let TouchPhase::Scroll { last_y } = ts.phase {
                // The scroll container is driven by blitz-shell's native scroll
                // mechanism; we update scroll_offset here so hit_test_document
                // stays accurate.
                // TODO(partial-render): replace with direct node.scroll_offset
                // once Blitz exposes it.
                let _ = last_y;
            }
        } else if ts.phase == TouchPhase::LongPress {
            let start = ts.start_pos;
            let (layout_opt, page_width_px, page_height_px) = {
                let Ok(state) = doc_state.lock() else { return };
                (
                    state.paginated_layout.clone(),
                    state.page_width_px,
                    state.page_height_px,
                )
            };
            if let Some(layout) = layout_opt {
                let x_off = (window_width() - page_width_px).max(0.0) / 2.0;
                let origin = (x_off, tokens::TOOLBAR_HEIGHT_TOP + tokens::SPACE_6);
                if let Some(pos) = hit_test_document(
                    start.0,
                    start.1,
                    origin,
                    scroll_offset(),
                    &layout,
                    page_width_px,
                    page_height_px,
                    page_gap_px,
                ) {
                    let ldoc_guard = loro_doc.read();
                    if let Some(ldoc) = ldoc_guard.as_ref() {
                        let text = get_block_text(ldoc, pos.paragraph_index);
                        if let Some((ws, we)) = word_boundaries_at(&text, pos.byte_offset) {
                            let anchor = DocumentPosition {
                                page_index: pos.page_index,
                                paragraph_index: pos.paragraph_index,
                                byte_offset: ws,
                            };
                            let focus = DocumentPosition {
                                page_index: pos.page_index,
                                paragraph_index: pos.paragraph_index,
                                byte_offset: we,
                            };
                            let mut cs = cursor_state.write();
                            cs.anchor = Some(anchor);
                            cs.focus = Some(focus);
                        }
                    }
                }
            }
        }
        touch_state.set(Some(ts));
    }
}

/// Builds the `ontouchend` handler for tap cursor placement.
pub(super) fn make_touchend_handler(
    doc_state: Arc<Mutex<DocumentState>>,
    mut touch_state: Signal<Option<TouchInteractionState>>,
    window_width: Signal<f32>,
    scroll_offset: Signal<f32>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    mut cursor_state: Signal<CursorState>,
    page_gap_px: f32,
) -> impl FnMut(TouchEvent) {
    move |_evt: TouchEvent| {
        let Some(ts) = touch_state() else { return };
        match ts.phase {
            TouchPhase::Indeterminate | TouchPhase::Tap => {
                // Short tap — place cursor via the same hit-test path as a mouse click.
                let (layout_opt, page_width_px, page_height_px) = {
                    let Ok(state) = doc_state.lock() else {
                        touch_state.set(None);
                        return;
                    };
                    (
                        state.paginated_layout.clone(),
                        state.page_width_px,
                        state.page_height_px,
                    )
                };
                if let Some(layout) = layout_opt {
                    let x_off = (window_width() - page_width_px).max(0.0) / 2.0;
                    let origin = (x_off, tokens::TOOLBAR_HEIGHT_TOP + tokens::SPACE_6);
                    if let Some(pos) = hit_test_document(
                        ts.start_pos.0,
                        ts.start_pos.1,
                        origin,
                        scroll_offset(),
                        &layout,
                        page_width_px,
                        page_height_px,
                        page_gap_px,
                    ) {
                        let loro_cursor = loro_doc.read().as_ref().and_then(|ldoc| {
                            derive_loro_cursor(ldoc, pos.paragraph_index, pos.byte_offset)
                        });
                        let mut cs = cursor_state.write();
                        cs.loro_cursor = loro_cursor;
                        cs.anchor = Some(pos.clone());
                        cs.focus = Some(pos);
                    }
                }
            }
            // Scroll and long-press states are already handled incrementally
            // in ontouchmove.
            TouchPhase::Scroll { .. } | TouchPhase::LongPress => {}
        }
        touch_state.set(None);
    }
}
