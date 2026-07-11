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
//!    column fits *at the current zoom*, reflowed otherwise, hysteretic to avoid
//!    thrash — until the user freezes the mode,
//! 3. publish the measured width **and the live zoom** into the shared
//!    `appthere_ui` responsive context so any component can read the derived
//!    [`Breakpoint`] and the page-fit rule scales with the user's zoom.
//!
//! [`Breakpoint`]: appthere_ui::Breakpoint

use std::sync::{Arc, Mutex};

use appthere_ui::{AtResponsiveContext, PageFit, Viewport, resolve_page_fit};
use dioxus::prelude::*;
use loki_renderer::ViewMode;

use super::editor_scrollbar::{CanvasMounted, ScrollMetrics};
use crate::editing::state::DocumentState;

/// Converts the status-bar zoom percentage (100 = 100%) into the [`Viewport`]
/// zoom fraction (1.0 = 100%) consumed by [`resolve_page_fit`].
pub(super) fn zoom_fraction(percent: u32) -> f32 {
    percent as f32 / 100.0
}

/// Resolves the page-fit view mode for a measured width, page geometry, and the
/// user's zoom, hysteretic on the current mode (Spec 03 M2 / D2).
///
/// A wider `zoom_percent` grows the page column, so a page that fit at 100% can
/// stop fitting when zoomed in and flip the editor to the reflow renderer rather
/// than force horizontal scrolling.
pub(super) fn desired_view_mode(
    width: f32,
    page_width_px: f32,
    zoom_percent: u32,
    currently_paginated: bool,
) -> ViewMode {
    let viewport = Viewport::new(width).with_zoom(zoom_fraction(zoom_percent));
    match resolve_page_fit(viewport, page_width_px, currently_paginated) {
        PageFit::Paginated => ViewMode::Paginated,
        PageFit::Reflow => ViewMode::Reflow,
    }
}

/// Wires the three viewport-driven effects (see the module docs).
pub(super) fn use_viewport_effects(
    canvas_mounted: CanvasMounted,
    scroll_metrics: Signal<ScrollMetrics>,
    doc_state: Arc<Mutex<DocumentState>>,
    mut view_mode: Signal<ViewMode>,
    view_mode_user_set: Signal<bool>,
    zoom_percent: Signal<u32>,
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
            // default if the state lock is unavailable. Reading `zoom_percent`
            // here subscribes this effect to zoom changes, so zooming a page past
            // the point it fits re-evaluates the renderer (Spec 03 M2, 4a.4).
            let page_width_px = doc_state
                .lock()
                .map_or(appthere_ui::tokens::PAGE_WIDTH_PX, |s| s.page_width_px);
            let currently_paginated = *view_mode.peek() == ViewMode::Paginated;
            let desired = desired_view_mode(
                width,
                page_width_px,
                *zoom_percent.read(),
                currently_paginated,
            );
            if *view_mode.peek() != desired {
                view_mode.set(desired);
            }
        });
    }

    // 3. Publish the measured width **and the live zoom** into the shared
    //    responsive context so the breakpoint derives from the same value the
    //    renderer/hit-test use and the page-fit rule (effect 2, and any other
    //    consumer of `Viewport::zoom`) scales with the user's zoom. DPI is
    //    preserved.
    let responsive = use_context::<AtResponsiveContext>();
    use_effect(move || {
        let width = scroll_metrics.read().client_width;
        let zoom = zoom_fraction(*zoom_percent.read());
        let mut viewport = responsive.viewport;
        let prev = *viewport.peek();
        if (prev.inner_width_px - width).abs() > f32::EPSILON
            || (prev.zoom - zoom).abs() > f32::EPSILON
        {
            viewport.set(Viewport {
                inner_width_px: width,
                zoom,
                ..prev
            });
        }
    });
}

#[cfg(test)]
#[path = "editor_responsive_tests.rs"]
mod tests;
