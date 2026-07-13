// SPDX-License-Identifier: Apache-2.0

//! Touch event handler factories for the document canvas (split from
//! `editor_pointer.rs` for the 300-line ceiling).
//!
//! Gestures: tap places the caret; a moved touch scrolls; a stationary touch
//! long-presses into word selection; and a touch that starts on a selection
//! handle drags that selection edge ([`TouchPhase::HandleDrag`] — the fixed
//! end is normalised into the anchor at grab time, so every move just updates
//! the focus, and the handles may cross).

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::loro_bridge::derive_loro_cursor;
use loki_doc_model::loro_mutation::get_block_text;
use loki_renderer::ViewMode;

use super::editor_pointer::reflow_tap_position;
use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::editing::hit_test::hit_test_document;
use crate::editing::selection_handles::grab_fixed_endpoint;
use crate::editing::state::DocumentState;
use crate::editing::touch::{TouchInteractionState, TouchPhase, word_boundaries_at};
use crate::editing::viewport::Viewport;
use crate::routes::editor::editor_scrollbar::ScrollMetrics;

/// Resolves a window-relative point to a paginated document position, using
/// the same origin/zoom transform as the mouse handlers. `None` when no
/// layout is loaded or the point misses every page.
fn paginated_hit(
    doc_state: &Arc<Mutex<DocumentState>>,
    point: (f32, f32),
    scroll_offset: f32,
    scroll_metrics: Signal<ScrollMetrics>,
    zoom_percent: Signal<u32>,
    page_gap_px: f32,
) -> Option<DocumentPosition> {
    let (layout_opt, page_width_px, page_height_px) = {
        let state = doc_state.lock().ok()?;
        (
            state.paginated_layout.clone(),
            state.page_width_px,
            state.page_height_px,
        )
    };
    let layout = layout_opt?;
    let zoom = zoom_percent() as f32 / 100.0;
    let viewport = Viewport::new(scroll_metrics.peek().client_width);
    let x_off = viewport.centred_origin_x(page_width_px * zoom);
    let origin = (x_off, tokens::TOOLBAR_HEIGHT_TOP + tokens::SPACE_6);
    hit_test_document(
        point.0,
        point.1,
        origin,
        scroll_offset,
        &layout,
        page_width_px,
        page_height_px,
        page_gap_px,
        zoom,
    )
}

/// Builds the `ontouchstart` handler: begins gesture classification, and —
/// when a range selection is active and the touch lands on one of its
/// teardrop handles — starts a handle drag instead.
#[allow(clippy::too_many_arguments)]
pub(super) fn make_touchstart_handler(
    doc_state: Arc<Mutex<DocumentState>>,
    mut touch_state: Signal<Option<TouchInteractionState>>,
    mut cursor_state: Signal<CursorState>,
    scroll_offset: Signal<f32>,
    scroll_metrics: Signal<ScrollMetrics>,
    view_mode: Signal<ViewMode>,
    zoom_percent: Signal<u32>,
    page_gap_px: f32,
) -> impl FnMut(TouchEvent) {
    move |evt: TouchEvent| {
        let touches = evt.touches();
        let Some(first) = touches.first() else { return };
        let c = first.client_coordinates();
        let pos = (c.x as f32, c.y as f32);
        let mut state = TouchInteractionState::new(0, pos);

        // Handle grab — paginated mode only (handles paint there).
        if *view_mode.peek() != ViewMode::Reflow && cursor_state.peek().has_selection() {
            let grabbed = {
                let cs = cursor_state.peek();
                let (Some(anchor), Some(focus)) = (cs.anchor.clone(), cs.focus.clone()) else {
                    touch_state.set(Some(state));
                    return;
                };
                let layout_and_dims = doc_state.lock().ok().map(|s| {
                    (
                        s.paginated_layout.clone(),
                        s.page_width_px,
                        s.page_height_px,
                    )
                });
                match layout_and_dims {
                    Some((Some(layout), page_width_px, page_height_px)) => {
                        let zoom = *zoom_percent.peek() as f32 / 100.0;
                        let viewport = Viewport::new(scroll_metrics.peek().client_width);
                        let x_off = viewport.centred_origin_x(page_width_px * zoom);
                        let origin = (x_off, tokens::TOOLBAR_HEIGHT_TOP + tokens::SPACE_6);
                        grab_fixed_endpoint(
                            &layout,
                            &anchor,
                            &focus,
                            pos,
                            origin,
                            *scroll_offset.peek(),
                            page_height_px,
                            page_gap_px,
                            zoom,
                        )
                    }
                    _ => None,
                }
            };
            if let Some(fixed) = grabbed {
                state.phase = TouchPhase::HandleDrag;
                cursor_state.write().anchor = Some(fixed);
            }
        }
        touch_state.set(Some(state));
    }
}

/// Builds the `ontouchmove` handler for scroll, long-press word selection, and
/// handle dragging.
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
    zoom_percent: Signal<u32>,
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
        } else if ts.phase == TouchPhase::HandleDrag {
            // Drag the selection focus under the finger (the grabbed edge).
            if let Some(p) = paginated_hit(
                &doc_state,
                new_pos,
                scroll_offset(),
                scroll_metrics,
                zoom_percent,
                page_gap_px,
            ) {
                cursor_state.write().focus = Some(p);
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
                paginated_hit(
                    &doc_state,
                    start,
                    scroll_offset(),
                    scroll_metrics,
                    zoom_percent,
                    page_gap_px,
                )
                .map(|p| (p.page_index, p.paragraph_index, p.byte_offset))
            };
            if let Some((page, para, byte)) = resolved {
                let ldoc_guard = loro_doc.read();
                if let Some(ldoc) = ldoc_guard.as_ref() {
                    let text = get_block_text(ldoc, para);
                    if let Some((ws, we)) = word_boundaries_at(&text, byte) {
                        let mut cs = cursor_state.write();
                        cs.anchor = Some(DocumentPosition::top_level(page, para, ws));
                        cs.focus = Some(DocumentPosition::top_level(page, para, we));
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
    zoom_percent: Signal<u32>,
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
                    let pos = DocumentPosition::top_level(0, para, byte);
                    let mut cs = cursor_state.write();
                    cs.loro_cursor = loro_cursor;
                    cs.anchor = Some(pos.clone());
                    cs.focus = Some(pos);
                }
            }
            TouchPhase::Indeterminate | TouchPhase::Tap => {
                // Short tap — place cursor via the same hit-test path as a mouse click.
                if let Some(pos) = paginated_hit(
                    &doc_state,
                    ts.start_pos,
                    scroll_offset(),
                    scroll_metrics,
                    zoom_percent,
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
            // Scroll and long-press are handled incrementally in ontouchmove;
            // a handle drag simply ends, keeping the adjusted selection.
            TouchPhase::Scroll { .. } | TouchPhase::LongPress | TouchPhase::HandleDrag => {}
        }
        touch_state.set(None);
    }
}
