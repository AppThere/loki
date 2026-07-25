// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Keep the caret on screen while typing (Spec 08 I-05, T1.3–T1.5).
//!
//! # Why a component and not a hook call
//!
//! The effect needs a hook scope, and `render_canvas_area` is a plain function
//! that cannot call hooks. Hosting it in `editor_inner` would grow a file that
//! is already over the 300-line ceiling and pinned by the CI ratchet, so this
//! is a zero-output sensor component mounted inside the canvas subtree instead
//! — the same shape as `SafeAreaResizeSensor` and `AtViewportWidthSensor`, and
//! what ADR-0013 prescribes for anything that needs its own hook scope.
//!
//! # What triggers a reveal (T1.4)
//!
//! The effect subscribes to `cursor_state`, so it runs when — and only when —
//! the caret or selection actually moves. That gives T1.4's rule almost for
//! free:
//!
//! - typing, deletion and arrow/Home/End navigation move the caret → reveal;
//! - a wheel or touch scroll does **not** move the caret → no reveal, so the
//!   viewport never fights a user who has scrolled away to read;
//! - drag-select does move the caret, so it is gated explicitly on
//!   `is_dragging`;
//! - clicking to place the caret moves it, but the click was inside the
//!   viewport, so `reveal_offset` finds it already visible and does nothing.
//!
//! # The soft keyboard (T1.5) needs no special case
//!
//! T1.5 requires the reveal to target the safe area rather than the window when
//! a soft keyboard is up. It already does, because the reveal measures against
//! the scroll container's **own measured** `client_height` rather than the
//! window: `routes::shell` sizes the shell `calc(100vh - inset_total)`, and the
//! Android inset query folds `WindowInsets.Type.ime()` into that total (S0.4),
//! so the container physically shrinks when the keyboard appears and the
//! measurement follows. Where no IME exists — desktop, or Android with a
//! hardware keyboard — the inset is zero and the safe area *is* the window,
//! which is exactly what T1.5 asks for. Adding a second, explicit safe-area
//! term here would double-count it.
//!
//! # Why there is no debounce timer
//!
//! T1.4 asks for debouncing during fast typing. None is needed, and adding one
//! would make the caret lag the text: `reveal_offset` is idempotent — it
//! returns `None` whenever the caret is already visible with its margins — and
//! when it does scroll it moves the **minimum** distance. Successive keystrokes
//! therefore produce either nothing or a few pixels of follow, never the
//! oscillation a debounce would be protecting against. Revisit if a device says
//! otherwise; it is a one-line change to `scroll_to`'s call site.

use std::sync::{Arc, Mutex};

use appthere_ui::{RevealMargin, ScrollMetrics, use_viewport_controller};
use dioxus::prelude::*;
use loki_renderer::ViewMode;
use loki_renderer::render_layout::{reflow_layout_content_width_pt, reflow_type_scale};

use super::editor_responsive::zoom_fraction;
use super::editor_scrollbar::CanvasMounted;
use crate::editing::caret_reveal::{
    PageStack, caret_line_height_px, caret_rect_paginated, caret_rect_reflow,
};
use crate::editing::cursor::CursorState;
use crate::editing::state::{DocumentState, ensure_reflow_layout};

/// Line height assumed when the caret has no measured rect yet (11 pt single
/// spaced at 100%). Only used for the reveal margin, never for placement.
const FALLBACK_LINE_PX: f32 = 18.0;

#[derive(Clone, Props)]
pub(super) struct CaretFollowProps {
    pub(super) doc_state: Arc<Mutex<DocumentState>>,
    pub(super) cursor_state: Signal<CursorState>,
    pub(super) scroll_metrics: Signal<ScrollMetrics>,
    pub(super) canvas_mounted: CanvasMounted,
    /// True while a pointer drag-select is in progress; suppresses the reveal.
    pub(super) is_dragging: Signal<bool>,
    pub(super) view_mode: Signal<ViewMode>,
    pub(super) zoom_percent: Signal<u32>,
    pub(super) page_gap_px: f32,
    /// Top padding of the scroll container, in CSS px.
    pub(super) content_top_px: f32,
}

impl PartialEq for CaretFollowProps {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.doc_state, &other.doc_state)
            && self.cursor_state == other.cursor_state
            && self.scroll_metrics == other.scroll_metrics
            && self.canvas_mounted == other.canvas_mounted
            && self.is_dragging == other.is_dragging
            && self.view_mode == other.view_mode
            && self.zoom_percent == other.zoom_percent
            && self.page_gap_px == other.page_gap_px
            && self.content_top_px == other.content_top_px
    }
}

/// Zero-output sensor that scrolls the caret into view when it moves.
// A Dioxus component must be PascalCase to be usable as `CaretFollow {}` in
// rsx. `#[component]` would supply this allow itself, but it derives the props
// struct from the argument list and so requires every prop to be `PartialEq` —
// `Arc<Mutex<DocumentState>>` is not, hence the hand-written props above with
// their `Arc::ptr_eq` comparison. Same trade as `PageTile` and `ReflowDocView`,
// which are the other two components in the workspace taking a props struct
// directly.
#[allow(non_snake_case)]
pub(super) fn CaretFollow(props: CaretFollowProps) -> Element {
    let mut controller = use_viewport_controller(props.scroll_metrics, props.canvas_mounted);
    let doc_state = Arc::clone(&props.doc_state);
    let cursor_state = props.cursor_state;
    let is_dragging = props.is_dragging;
    let view_mode = props.view_mode;
    let zoom_percent = props.zoom_percent;
    let metrics = props.scroll_metrics;
    let page_gap_px = props.page_gap_px;
    let content_top_px = props.content_top_px;

    use_effect(move || {
        // Subscribes this effect to caret movement — the whole trigger rule.
        let focus = cursor_state.read().focus.clone();
        let Some(focus) = focus else { return };
        // T1.4: a drag is the user choosing where to look; do not move it.
        if *is_dragging.peek() {
            return;
        }
        let m = *metrics.peek();
        if !m.is_measured() {
            return;
        }

        let reflow = *view_mode.peek() == ViewMode::Reflow;
        let rect = if reflow {
            let width_px = m.client_width;
            if width_px <= 1.0 {
                return;
            }
            let scale = reflow_type_scale(width_px);
            let content_w = reflow_layout_content_width_pt(width_px);
            let Some(layout) = ensure_reflow_layout(&doc_state, content_w) else {
                return;
            };
            caret_rect_reflow(
                &layout,
                focus.paragraph_index,
                focus.byte_offset,
                scale,
                content_top_px,
            )
        } else {
            // One lock for both reads: taking it twice could observe a layout
            // and a page height from either side of a relayout, which is how a
            // caret lands one page off.
            let (layout_opt, page_height_px) = {
                let Ok(state) = doc_state.lock() else { return };
                (state.paginated_layout.clone(), state.page_height_px)
            };
            let Some(layout) = layout_opt else { return };
            let stack = PageStack {
                page_height_px,
                page_gap_px,
                zoom: zoom_fraction(*zoom_percent.peek()),
                content_top_px,
            };
            caret_rect_paginated(&layout, &focus, stack)
        };

        let Some(rect) = rect else { return };
        // T1.3: the margin is three body lines below and one above, measured
        // from the caret's own line so it holds at every size and zoom.
        let line = caret_line_height_px(Some(rect), FALLBACK_LINE_PX);
        // Instant, always. A smooth caret-follow would lag the text at typing
        // speed; smooth is reserved for discrete jumps (Find, Go To Page).
        controller.scroll_to_reveal(
            rect,
            RevealMargin::caret_lines(line),
            ScrollBehavior::Instant,
        );
    });

    rsx! {}
}
