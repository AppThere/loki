// SPDX-License-Identifier: Apache-2.0

//! The canvas scroll handler (split from `editor_canvas.rs`, CLAUDE.md
//! technique 3 — that file is baselined over the 300-line ceiling).
//!
//! Scroll events are dispatched by the patched Blitz shell (PATCH(loki) in
//! blitz-shell/blitz-dom/dioxus-native-dom) after a wheel or touch gesture
//! changes the container's scroll offset. The handler mirrors the full scroll
//! geometry for the custom scrollbars and updates the status-bar page
//! indicator: the current page is the one occupying the vertical centre of
//! the viewport.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;

use super::editor_scrollbar::ScrollMetrics;
use crate::editing::state::DocumentState;

/// Builds the `onscroll` closure for the canvas scroll container.
pub(super) fn make_scroll_handler(
    doc_state_scroll: Arc<Mutex<DocumentState>>,
    mut scroll_offset: Signal<f32>,
    mut scroll_metrics: Signal<ScrollMetrics>,
    mut current_page: Signal<u32>,
    zoom_percent: Signal<u32>,
    page_gap_px: f32,
) -> impl FnMut(ScrollEvent) {
    move |evt: ScrollEvent| {
        let top = evt.scroll_top() as f32;
        scroll_offset.set(top);
        let viewport_h = evt.client_height() as f32;
        // scroll_width / scroll_height are the scrollable distance
        // (content − client); see `editor_scrollbar`.
        scroll_metrics.set(ScrollMetrics {
            scroll_top: top,
            scroll_left: evt.scroll_left() as f32,
            scroll_width: evt.scroll_width() as f32,
            scroll_height: evt.scroll_height() as f32,
            client_width: evt.client_width() as f32,
            client_height: viewport_h,
        });
        let (page_h, count) = match doc_state_scroll.lock() {
            Ok(s) => (s.page_height_px, s.page_count),
            Err(_) => return,
        };
        // Tiles are painted at `zoom` scale (the inter-page gap is a fixed,
        // unscaled CSS margin), so the page stride the scroll offset measures
        // against is `page_h × zoom + gap`.
        let zoom = zoom_percent() as f32 / 100.0;
        let slot = page_h * zoom + page_gap_px;
        if slot <= 0.0 || count == 0 {
            return;
        }
        let page =
            (((top + viewport_h * 0.5) / slot).floor() as i64 + 1).clamp(1, count as i64) as u32;
        if *current_page.peek() != page {
            current_page.set(page);
        }
    }
}
