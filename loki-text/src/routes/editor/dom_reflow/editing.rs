// SPDX-License-Identifier: Apache-2.0

//! Click-to-place-caret and the caret overlay for the DOM reflow view
//! (ADR-0017 sequencing step 12, first slice of `TODO(dom-reflow-editing)`).
//!
//! # Reused, not reinvented
//!
//! The canvas-based reflow view already has a full-document
//! [`loki_layout::ContinuousLayout`] built purely for hit-testing and caret
//! geometry ([`ensure_reflow_layout`]), with `hit_test` and
//! `cursor_rect_canvas` both already tested against the touch and mouse
//! click paths. This reuses the same layout and the same two queries rather
//! than shaping a second time — the only new part is how the click's local
//! coordinates are obtained.
//!
//! # Why no Strategy-C origin computation is needed here
//!
//! `crate::editing::hit_test`'s "Strategy C" exists because
//! `MountedData::get_client_rect()` / `offset_x` / `offset_y` are
//! `unimplemented!()` for a **canvas** element in this Blitz build. The DOM
//! reflow view paints ordinary `div`/`span` elements, for which
//! `element_coordinates()` **is** implemented (`patches/dioxus-native-dom`) —
//! it reports the click position relative to the target element's own
//! padding edge, "as on the web". [`super::dom_reflow_view`]'s inner column
//! div carries no padding of its own (padding lives on its *parent*, for
//! centring), so that origin already coincides with `ContinuousLayout`'s
//! (0, 0): no computed offset, no scroll-position bookkeeping, just a
//! px→pt conversion.
use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_doc_model::loro_bridge::derive_loro_cursor;
use loki_layout::ContinuousLayout;

use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::editing::state::{DocumentState, ensure_reflow_layout};

/// CSS pixels → layout points (72 dpi / 96 dpi) — the same ratio
/// `crate::editing::hit_test` uses.
const PX_TO_PT: f32 = 72.0 / 96.0;

/// The pure half of [`place_caret_at_click`]: converts an element-local click
/// point (CSS px) into a [`DocumentPosition`], against an already-built
/// layout. Split out so it is testable without a Dioxus runtime — `Signal`
/// cannot be constructed outside one, which is why the Signal-writing half
/// stays a thin, untested wrapper (the same split this crate uses throughout
/// `editing::hit_test` vs. `routes::editor::editor_pointer`).
#[must_use]
fn resolve_click_position(
    layout: &ContinuousLayout,
    element_x: f32,
    element_y: f32,
) -> Option<DocumentPosition> {
    let (block_index, byte_offset) = layout.hit_test(element_x * PX_TO_PT, element_y * PX_TO_PT)?;
    Some(DocumentPosition::top_level(0, block_index, byte_offset))
}

/// Everything a DOM reflow view interaction needs to reach the document and
/// its cursor/undo state. Cloned into each event closure rather than
/// borrowed — the signals are `Copy` and `doc_state` is an `Arc`, so cloning
/// is cheap, and a `'static` Dioxus event handler cannot borrow from the
/// render call that built it.
#[derive(Clone)]
pub(super) struct EditingCtx {
    pub(super) doc_state: Arc<Mutex<DocumentState>>,
    pub(super) cursor_state: Signal<CursorState>,
    pub(super) loro_doc: Signal<Option<loro::LoroDoc>>,
    pub(super) undo_manager: Signal<Option<loro::UndoManager>>,
    pub(super) can_undo: Signal<bool>,
    pub(super) can_redo: Signal<bool>,
    pub(super) save_request: Signal<u32>,
}

/// Resolves a click on the reading column to a document position and places
/// the caret there (collapsed selection: anchor = focus).
///
/// `element_x` / `element_y` are
/// [`dioxus::html::input_data::MouseData::element_coordinates`], in CSS
/// pixels, relative to the column's own padding edge — see the module docs
/// for why that is already `ContinuousLayout`'s origin. `max_w` must be the
/// same content width the column is currently rendered at
/// ([`super::column_max_width_pt`]), or the hit-test disagrees with what is
/// on screen (the general rule `reflow_hit_test_window` states for the
/// canvas path too).
///
/// No-op when no document is loaded or the point does not resolve (e.g. a
/// click below the last paragraph on a not-yet-laid-out frame).
pub(super) fn place_caret_at_click(ctx: &EditingCtx, element_x: f32, element_y: f32, max_w: f32) {
    let Some(layout) = ensure_reflow_layout(&ctx.doc_state, max_w) else {
        return;
    };
    // `top_level`, via `resolve_click_position`: the DOM reflow view does not
    // yet address nested containers (table cells, note bodies) —
    // `TODO(dom-reflow-editing)` covers that alongside selection and the
    // other deferred pieces.
    let Some(pos) = resolve_click_position(&layout, element_x, element_y) else {
        return;
    };
    let loro_cursor = ctx
        .loro_doc
        .read()
        .as_ref()
        .and_then(|ldoc| derive_loro_cursor(ldoc, pos.paragraph_index, pos.byte_offset));
    let mut cursor_state = ctx.cursor_state;
    let mut cs = cursor_state.write();
    cs.loro_cursor = loro_cursor;
    cs.anchor = Some(pos.clone());
    cs.focus = Some(pos);
}

/// The caret overlay: an absolutely-positioned line at the current focus
/// position, or nothing when no cursor is placed or the position does not
/// resolve against this layout (e.g. a nested position carried over from the
/// paginated view, which this hit-test does not address).
///
/// Positioned via CSS inside the column's `position: relative` inner div —
/// an absolutely positioned child's `left`/`top` resolve against its
/// containing block's *padding* edge, which is the same origin
/// `cursor_rect_canvas` uses (see the module docs), so no further coordinate
/// conversion happens here.
pub(super) fn caret_el(ctx: &EditingCtx, max_w: f32) -> Element {
    let focus = ctx.cursor_state.read().focus.clone();
    let Some(focus) = focus else {
        return rsx! {};
    };
    let Some(layout) = ensure_reflow_layout(&ctx.doc_state, max_w) else {
        return rsx! {};
    };
    let Some(rect) = layout.cursor_rect_canvas(focus.paragraph_index, focus.byte_offset) else {
        return rsx! {};
    };
    rsx! {
        div { style: caret_css(&rect) }
    }
}

/// The caret overlay's CSS — split out from [`caret_el`] so it is testable
/// without a Dioxus runtime.
///
/// 2 pt wide, matching `loki-vello::scene_cursor`'s canvas-painted caret
/// width. `pointer-events: none` so the caret itself is never the click
/// target — a click "on" the caret must still hit-test against the text
/// underneath it.
#[must_use]
fn caret_css(rect: &loki_layout::CursorRect) -> String {
    format!(
        "position: absolute; left: {x}pt; top: {y}pt; width: 2pt; \
         height: {h}pt; background: #000000; pointer-events: none;",
        x = rect.x,
        y = rect.y,
        h = rect.height,
    )
}

#[cfg(test)]
#[path = "editing_tests.rs"]
mod tests;
