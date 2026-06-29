// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Viewport-driven editor effects (Spec 03 M1).
//!
//! Extracted from `editor_inner` so the responsive wiring lives in one place and
//! the over-ceiling `editor_inner` shrinks rather than grows. All three effects
//! derive from the **one** measured scroll-container width — there is no second
//! width source (Spec 01 A-1 / Spec 03 §2):
//!
//! 1. seed `scroll_metrics` from `get_client_rect` at mount (so the width is
//!    known before the first scroll),
//! 2. choose the renderer by **page-fit** (Spec 03 M2) — paginated when a page
//!    column fits, reflowed otherwise, hysteretic to avoid thrash — until the
//!    user freezes the mode,
//! 3. publish the measured width into the shared `appthere_ui` responsive
//!    context so any component can read the derived [`Breakpoint`].
//!
//! [`Breakpoint`]: appthere_ui::Breakpoint

use std::sync::{Arc, Mutex};

use appthere_ui::{AtResponsiveContext, PageFit, Viewport, resolve_page_fit};
use dioxus::prelude::*;
use loki_renderer::ViewMode;

use super::editor_scrollbar::{CanvasMounted, ScrollMetrics};
use crate::editing::state::DocumentState;

/// Wires the three viewport-driven effects (see the module docs).
pub(super) fn use_viewport_effects(
    canvas_mounted: CanvasMounted,
    scroll_metrics: Signal<ScrollMetrics>,
    doc_state: Arc<Mutex<DocumentState>>,
    mut view_mode: Signal<ViewMode>,
    view_mode_user_set: Signal<bool>,
) {
    // 1. Seed the metrics at mount. Otherwise `client_width` stays 0 until the
    //    first DOM scroll, leaving the view-mode default and reflow width unknown.
    use_effect(move || {
        let Some(evt) = canvas_mounted() else { return };
        if scroll_metrics.peek().client_width > 0.0 {
            return;
        }
        let mut metrics = scroll_metrics;
        spawn(async move {
            if let Ok(rect) = evt.get_client_rect().await {
                let mut m = metrics.write();
                if m.client_width <= 0.0 {
                    m.client_width = rect.size.width as f32;
                    m.client_height = rect.size.height as f32;
                }
            }
        });
    });

    // 2. Default the view mode by **page fit** (Spec 03 M2 / D2) — paginated when
    //    a full page column fits the measured viewport at the current zoom,
    //    reflowed when it would force horizontal scrolling — until the user picks
    //    a mode (which freezes this default). The decision is hysteretic on the
    //    *current* mode, so dragging across the boundary doesn't thrash.
    {
        let doc_state = Arc::clone(&doc_state);
        use_effect(move || {
            if *view_mode_user_set.read() {
                return;
            }
            let width = scroll_metrics.read().client_width;
            if width <= 0.0 {
                return;
            }
            // Page geometry is per-document (CSS px); falls back to the A4 token
            // default if the state lock is unavailable. Zoom is fixed at 100%
            // until zoom is implemented — `resolve_page_fit` already scales by
            // `Viewport::zoom`, so this becomes live for free when it lands.
            let page_width_px = doc_state
                .lock()
                .map_or(appthere_ui::tokens::PAGE_WIDTH_PX, |s| s.page_width_px);
            let currently_paginated = *view_mode.peek() == ViewMode::Paginated;
            let desired =
                match resolve_page_fit(Viewport::new(width), page_width_px, currently_paginated) {
                    PageFit::Paginated => ViewMode::Paginated,
                    PageFit::Reflow => ViewMode::Reflow,
                };
            if *view_mode.peek() != desired {
                view_mode.set(desired);
            }
        });
    }

    // 3. Publish the measured width into the shared responsive context so the
    //    breakpoint derives from the same value the renderer/hit-test use. Zoom
    //    and DPI are preserved for the Spec 03 M2 page-fit switch to populate.
    let responsive = use_context::<AtResponsiveContext>();
    use_effect(move || {
        let width = scroll_metrics.read().client_width;
        let mut viewport = responsive.viewport;
        let prev = *viewport.peek();
        if (prev.inner_width_px - width).abs() > f32::EPSILON {
            viewport.set(Viewport {
                inner_width_px: width,
                ..prev
            });
        }
    });
}
