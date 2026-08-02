// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The editor's side of the zoom control: the three computed zooms and the
//! capability cap the readout reports (Spec 08 T5.4, T5.5).
//!
//! # Why the fits live here and not in `appthere_ui`
//!
//! The arithmetic is in `appthere_ui::components::zoom::fit`, where it is pure
//! and tested. What is *here* is the measurement: which page, and how big is the
//! viewport. Those are the editor's, and passing them up into the design system
//! would make a shared component depend on this app's layout — the same boundary
//! ADR-0013 draws for panels.
//!
//! # The cap is read through the same function the renderer applies
//!
//! `loki_renderer::zoom_capability::capability_limit_permille` is called here
//! *and* in `DocumentView`. Two call sites, one derivation — which is the
//! distinction Spec 08 T5.4 requirement 1 actually draws. Recomputing the bound
//! from `ZOOM_RANGE_MAX` or from a remembered integer would be the second
//! derivation, and it is the mistake the requirement was written against.

use std::sync::{Arc, Mutex};

use appthere_ui::scroll::{ScrollMetrics, ViewportController, ZoomAnchor};
use appthere_ui::{actual_size_zoom_percent, fit_page_zoom_percent, fit_width_zoom_percent};
use dioxus::prelude::*;

use crate::editing::state::DocumentState;

/// Horizontal breathing room either side of the page when fitting, in CSS px.
///
/// The page is centred in the canvas with a gutter; fitting edge-to-edge would
/// put the page against the scrollbar and read as an overflow rather than a fit.
const FIT_MARGIN_PX: f32 = 48.0;

/// The measurements a fit needs.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FitInputs {
    /// The page's width in CSS px at 100% zoom — `DocumentState::page_width_px`,
    /// which the layout already converted from points.
    ///
    /// **The laid-out page, not the largest one in the document.** Deliberately
    /// different from the capability bound's choice, which takes the largest page
    /// because a limit that moved under a scroll would change the effective zoom
    /// with no user action. A *fit* is a one-shot command: the user pressed it
    /// while looking at a page, and fitting the document's biggest page would
    /// visibly under-fit the one on screen.
    pub page_width_px: f32,
    /// The page's height in CSS px at 100% zoom.
    pub page_height_px: f32,
    /// Measured viewport width in CSS px; `<= 0` before the canvas is measured.
    pub viewport_width_px: f32,
    /// Measured viewport height in CSS px.
    pub viewport_height_px: f32,
}

/// The zoom that fits the page width, or `None` before the canvas is measured.
#[must_use]
pub fn fit_width_percent(inputs: FitInputs) -> Option<u32> {
    fit_width_zoom_percent(
        inputs.viewport_width_px,
        inputs.page_width_px,
        FIT_MARGIN_PX,
    )
}

/// The zoom that fits the whole page.
#[must_use]
pub fn fit_page_percent(inputs: FitInputs) -> Option<u32> {
    fit_page_zoom_percent(
        inputs.viewport_width_px,
        inputs.viewport_height_px,
        inputs.page_width_px,
        inputs.page_height_px,
        FIT_MARGIN_PX,
    )
}

/// The zoom at which a document inch measures a physical inch, or `None` when
/// the platform has not reported a display density (T5.5).
#[must_use]
pub fn actual_size_percent(css_px_per_inch: Option<f32>) -> Option<u32> {
    actual_size_zoom_percent(css_px_per_inch)
}

/// The fit measurements for the document currently laid out, or `None` before
/// the layout or the canvas has produced a real one.
///
/// A poisoned lock yields `None` for the same reason an unmeasured canvas does:
/// the honest answer to "what should Fit Width be" is that we do not know, and
/// the row is hidden rather than wired to a guess.
pub(super) fn fit_inputs(
    doc_state: &Arc<Mutex<DocumentState>>,
    metrics: ScrollMetrics,
) -> Option<FitInputs> {
    let (w, h) = {
        let st = doc_state.lock().ok()?;
        (st.page_width_px, st.page_height_px)
    };
    (w > 1.0 && h > 1.0).then_some(FitInputs {
        page_width_px: w,
        page_height_px: h,
        viewport_width_px: metrics.client_width,
        viewport_height_px: metrics.client_height,
    })
}

/// The capability cap the renderer is applying, in permille.
///
/// Calls `loki_renderer::zoom_capability` rather than deriving anything — see
/// the module note. The page list is the laid-out document's, so the readout's
/// indicator and the renderer's cap are answers to the same question.
pub(super) fn capability_permille(doc_state: &Arc<Mutex<DocumentState>>) -> Option<u16> {
    let pages = {
        let st = doc_state.lock().ok()?;
        let layout = st.paginated_layout.as_ref()?;
        vec![
            (
                f64::from(layout.page_size.width),
                f64::from(layout.page_size.height),
            );
            layout.pages.len()
        ]
    };
    loki_renderer::zoom_capability::capability_limit_permille(
        &pages,
        crate::texture_budget::device_scale_factor(),
        crate::texture_budget::current(),
    )
}

/// The only way to change the zoom (Spec 08 T5.6).
///
/// # A capability, not a convention
///
/// Anchoring has to happen at **every** zoom change — the buttons, the presets,
/// the three fits, Actual Size, and the calibration that completes it. A rule
/// saying "remember to anchor" would be followed five times and forgotten once,
/// and the one that was forgotten is invisible: the page still zooms, it just
/// jumps somewhere else, which reads as the app being twitchy rather than as a
/// missing call.
///
/// So the zoom signal is not handed out. A caller gets this, and the only thing
/// it can do is set a zoom *with* an anchor (evidence rule 5: make the wrong
/// thing unavailable rather than documented).
///
/// # It is a trigger, and subscribes to nothing
///
/// I-20 was a reveal keyed on a scroll-derived quantity, which made the wheel
/// fight it. The tempting way to anchor is to widen the caret-follow effect so
/// it also watches zoom; that reintroduces I-20 exactly. This runs *in the
/// handler that changed the zoom*, reads the metrics with `metrics_now` (the
/// non-subscribing read), and issues one `scroll_to`. No effect gains a
/// dependency.
/// `PartialEq` compares the zoom it carries, not the container it scrolls.
///
/// Props need it, and the honest answer is that two commands are the same when
/// they would do the same thing — which is the zoom. Deriving it would compare
/// `ViewportController`'s four signals, none of which change identity, so the
/// derived answer would be "always equal" and a zoom change would not re-render
/// the status bar that displays it.
#[derive(Clone, Copy)]
pub(super) struct ZoomCommand {
    percent: Signal<u32>,
    viewport: ViewportController,
}

impl PartialEq for ZoomCommand {
    fn eq(&self, other: &Self) -> bool {
        *self.percent.read() == *other.percent.read()
    }
}

impl ZoomCommand {
    /// Wraps the zoom signal and the scroll container it applies to.
    pub(super) fn new(percent: Signal<u32>, viewport: ViewportController) -> Self {
        Self { percent, viewport }
    }

    /// The zoom currently requested.
    pub(super) fn percent(self) -> u32 {
        (self.percent)()
    }

    /// Sets the zoom, holding `anchor` still.
    ///
    /// The ratio is computed from **percent**, which is the requested zoom on
    /// both sides — so the two are the same quantity even when a capability cap
    /// is in force. Anchoring against the *effective* zoom would be more
    /// literally correct and is not available here: the cap is applied inside the
    /// renderer, after this runs. The error it costs is bounded by the cap and
    /// only appears on a device already reducing quality; mixing requested and
    /// effective would be an unbounded error on every device.
    /// TODO(t5.6-effective-anchor): anchor on the effective zoom once the
    /// renderer can report the pair atomically.
    pub(super) fn set(mut self, next_percent: u32, anchor: ZoomAnchor) {
        let current = *self.percent.peek();
        if next_percent == current {
            return;
        }
        self.percent.set(next_percent);

        self.viewport
            .zoom_to(anchor, current as f32 / 100.0, next_percent as f32 / 100.0);
    }
}

#[cfg(test)]
#[path = "editor_zoom_tests.rs"]
mod tests;
