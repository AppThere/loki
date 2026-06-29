// SPDX-License-Identifier: Apache-2.0

//! Mouse and touch event handler factories for the document canvas.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::loro_bridge::derive_loro_cursor;
use loki_doc_model::loro_mutation::get_block_text;

use loki_renderer::ViewMode;
use loki_renderer::render_layout::{reflow_content_width_pt, reflow_tile_width_px};

use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::editing::hit_test::{hit_test_document, reflow_hit_test_window};
use crate::editing::state::{DocumentState, ensure_reflow_layout};
use crate::editing::touch::{TouchInteractionState, TouchPhase, word_boundaries_at};
use crate::editing::viewport::Viewport;
use crate::routes::editor::editor_scrollbar::ScrollMetrics;

/// Resolves a window-relative tap to a reflow document position, using the same
/// continuous layout width as the painted view. Returns `None` outside reflow
/// mode or when the canvas has not been measured yet.
fn reflow_tap_position(
    doc_state: &Arc<Mutex<DocumentState>>,
    client_pos: (f32, f32),
    viewport: Viewport,
    scroll_offset: f32,
) -> Option<(usize, usize)> {
    let client_width_px = viewport.inner_width_px;
    if client_width_px <= 1.0 {
        return None;
    }
    let content_w = reflow_content_width_pt(client_width_px);
    let layout = ensure_reflow_layout(doc_state, content_w)?;
    // Reflow tiles are capped to a reading measure and centred in the viewport
    // (`margin: auto` on paint); the hit-test origin uses the same tile width so
    // clicks land on the painted glyphs (Spec 03 M4).
    let x_off = viewport.centred_origin_x(reflow_tile_width_px(client_width_px));
    let origin = (x_off, tokens::TOOLBAR_HEIGHT_TOP + tokens::SPACE_6);
    reflow_hit_test_window(client_pos.0, client_pos.1, origin, scroll_offset, &layout)
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
        let viewport = Viewport::new(scroll_metrics.peek().client_width);
        let x_off = viewport.centred_origin_x(page_width_px);
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
#[allow(clippy::too_many_arguments)]
pub(super) fn make_touchmove_handler(
    doc_state: Arc<Mutex<DocumentState>>,
    mut touch_state: Signal<Option<TouchInteractionState>>,
    scroll_offset: Signal<f32>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    mut cursor_state: Signal<CursorState>,
    page_gap_px: f32,
    view_mode: Signal<ViewMode>,
    scroll_metrics: Signal<ScrollMetrics>,
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
            // Resolve the long-press to a (page, paragraph, byte) position via the
            // view mode's hit-test path (reflow has no paginated layout).
            let resolved: Option<(usize, usize, usize)> = if view_mode() == ViewMode::Reflow {
                reflow_tap_position(
                    &doc_state,
                    start,
                    Viewport::new(scroll_metrics.peek().client_width),
                    scroll_offset(),
                )
                .map(|(para, byte)| (0, para, byte))
            } else {
                let (layout_opt, page_width_px, page_height_px) = {
                    let Ok(state) = doc_state.lock() else { return };
                    (
                        state.paginated_layout.clone(),
                        state.page_width_px,
                        state.page_height_px,
                    )
                };
                layout_opt.and_then(|layout| {
                    let viewport = Viewport::new(scroll_metrics.peek().client_width);
                    let x_off = viewport.centred_origin_x(page_width_px);
                    let origin = (x_off, tokens::TOOLBAR_HEIGHT_TOP + tokens::SPACE_6);
                    hit_test_document(
                        start.0,
                        start.1,
                        origin,
                        scroll_offset(),
                        &layout,
                        page_width_px,
                        page_height_px,
                        page_gap_px,
                    )
                    .map(|p| (p.page_index, p.paragraph_index, p.byte_offset))
                })
            };
            if let Some((page, para, byte)) = resolved {
                let ldoc_guard = loro_doc.read();
                if let Some(ldoc) = ldoc_guard.as_ref() {
                    let text = get_block_text(ldoc, para);
                    if let Some((ws, we)) = word_boundaries_at(&text, byte) {
                        let mut cs = cursor_state.write();
                        cs.anchor = Some(DocumentPosition {
                            page_index: page,
                            paragraph_index: para,
                            byte_offset: ws,
                        });
                        cs.focus = Some(DocumentPosition {
                            page_index: page,
                            paragraph_index: para,
                            byte_offset: we,
                        });
                    }
                }
            }
        }
        touch_state.set(Some(ts));
    }
}

/// Builds the `ontouchend` handler for tap cursor placement.
#[allow(clippy::too_many_arguments)]
pub(super) fn make_touchend_handler(
    doc_state: Arc<Mutex<DocumentState>>,
    mut touch_state: Signal<Option<TouchInteractionState>>,
    scroll_offset: Signal<f32>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    mut cursor_state: Signal<CursorState>,
    page_gap_px: f32,
    view_mode: Signal<ViewMode>,
    scroll_metrics: Signal<ScrollMetrics>,
) -> impl FnMut(TouchEvent) {
    move |_evt: TouchEvent| {
        let Some(ts) = touch_state() else { return };
        match ts.phase {
            // Reflow mode has no paginated layout: hit-test the continuous flow.
            TouchPhase::Indeterminate | TouchPhase::Tap if view_mode() == ViewMode::Reflow => {
                if let Some((para, byte)) = reflow_tap_position(
                    &doc_state,
                    ts.start_pos,
                    Viewport::new(scroll_metrics.peek().client_width),
                    scroll_offset(),
                ) {
                    let loro_cursor = loro_doc
                        .read()
                        .as_ref()
                        .and_then(|ldoc| derive_loro_cursor(ldoc, para, byte));
                    let pos = DocumentPosition {
                        page_index: 0,
                        paragraph_index: para,
                        byte_offset: byte,
                    };
                    let mut cs = cursor_state.write();
                    cs.loro_cursor = loro_cursor;
                    cs.anchor = Some(pos.clone());
                    cs.focus = Some(pos);
                }
            }
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
                    let viewport = Viewport::new(scroll_metrics.peek().client_width);
                    let x_off = viewport.centred_origin_x(page_width_px);
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
