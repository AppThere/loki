// SPDX-License-Identifier: Apache-2.0

//! Mouse event handler factories for the document canvas (touch handlers
//! live in `editor_pointer_touch.rs`).

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;

use loki_renderer::ViewMode;
use loki_renderer::render_layout::{
    reflow_layout_content_width_pt, reflow_tile_width_px, reflow_type_scale,
};

use crate::editing::cursor::CursorState;
use crate::editing::hit_test::{hit_test_document, reflow_hit_test_window};
use crate::editing::state::{DocumentState, ensure_reflow_layout};
use crate::editing::viewport::Viewport;
use crate::routes::editor::editor_scrollbar::ScrollMetrics;

/// Resolves a window-relative tap to a reflow document position, using the same
/// continuous layout width as the painted view. Returns `None` outside reflow
/// mode or when the canvas has not been measured yet.
pub(super) fn reflow_tap_position(
    doc_state: &Arc<Mutex<DocumentState>>,
    client_pos: (f32, f32),
    viewport: Viewport,
    scroll_offset: f32,
) -> Option<(usize, usize)> {
    let client_width_px = viewport.inner_width_px;
    if client_width_px <= 1.0 {
        return None;
    }
    let content_w = reflow_layout_content_width_pt(client_width_px);
    let layout = ensure_reflow_layout(doc_state, content_w)?;
    // Reflow tiles are capped to a reading measure and centred in the viewport
    // (`margin: auto` on paint); the hit-test origin uses the same tile width
    // (unchanged by the type scale — layout width and paint zoom cancel) so
    // clicks land on the painted glyphs (Spec 03 M4).
    let x_off = viewport.centred_origin_x(reflow_tile_width_px(client_width_px));
    let origin = (x_off, tokens::TOOLBAR_HEIGHT_TOP + tokens::SPACE_6);
    reflow_hit_test_window(
        client_pos.0,
        client_pos.1,
        origin,
        scroll_offset,
        reflow_type_scale(client_width_px),
        &layout,
    )
}

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
    scroll_metrics: Signal<ScrollMetrics>,
    scroll_offset: Signal<f32>,
    mut cursor_state: Signal<CursorState>,
    page_gap_px: f32,
    view_mode: Signal<ViewMode>,
    zoom_percent: Signal<u32>,
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
        // Tiles are painted at `zoom` scale and centred at that painted width, so
        // the centring origin and the hit-test both use the zoom factor.
        let zoom = zoom_percent() as f32 / 100.0;
        let viewport = Viewport::new(scroll_metrics.peek().client_width);
        let x_off = viewport.centred_origin_x(page_width_px * zoom);
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
            zoom,
        ) {
            cursor_state.write().focus = Some(p);
        }
    }
}

/// Builds the `onmousedown` handler: records the drag origin for the LEFT
/// button only. Right-click is handled per-tile (`on_tile_context` on
/// `DocumentView`), which has accurate `element_coordinates`; it is ignored
/// here so it does not start a spurious drag.
pub(super) fn make_mousedown_handler(
    mut drag_origin: Signal<Option<(f32, f32)>>,
) -> impl FnMut(MouseEvent) {
    move |evt: MouseEvent| {
        if evt.trigger_button() == Some(MouseButton::Secondary) {
            return;
        }
        let c = evt.client_coordinates();
        drag_origin.set(Some((c.x as f32, c.y as f32)));
    }
}

/// Builds the `onmouseup` handler: ends a drag-selection gesture.
pub(super) fn make_mouseup_handler(
    mut is_dragging: Signal<bool>,
    mut drag_origin: Signal<Option<(f32, f32)>>,
) -> impl FnMut(MouseEvent) {
    move |_evt: MouseEvent| {
        is_dragging.set(false);
        drag_origin.set(None);
    }
}
