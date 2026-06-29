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
//! 2. default the view mode by width until the user freezes it,
//! 3. publish the measured width into the shared `appthere_ui` responsive
//!    context so any component can read the derived [`Breakpoint`].
//!
//! [`Breakpoint`]: appthere_ui::Breakpoint

use appthere_ui::{AtResponsiveContext, Viewport};
use dioxus::prelude::*;
use loki_renderer::ViewMode;

use super::editor_scrollbar::{CanvasMounted, ScrollMetrics};

/// Viewport width (logical px) below which the editor defaults to the
/// reflowable view: a US-Letter page (~816px) plus margins no longer fits, so
/// paginated view would otherwise force horizontal scrolling. The user can
/// still toggle back to paginated.
///
/// Spec 03 M2 replaces this width guess with a real page-fit computation
/// (page geometry + zoom from the shared `Viewport`); until then it remains the
/// renderer-switch threshold.
const REFLOW_BREAKPOINT_PX: f32 = 900.0;

/// Wires the three viewport-driven effects (see the module docs).
pub(super) fn use_viewport_effects(
    canvas_mounted: CanvasMounted,
    scroll_metrics: Signal<ScrollMetrics>,
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

    // 2. Default the view mode by width — paginated when a page fits, reflowed
    //    when narrow — until the user picks a mode (which freezes this default).
    use_effect(move || {
        if *view_mode_user_set.read() {
            return;
        }
        let width = scroll_metrics.read().client_width;
        if width <= 0.0 {
            return;
        }
        let desired = if width < REFLOW_BREAKPOINT_PX {
            ViewMode::Reflow
        } else {
            ViewMode::Paginated
        };
        if *view_mode.peek() != desired {
            view_mode.set(desired);
        }
    });

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
