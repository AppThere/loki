// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Responsive foundation (Spec 03 M1): the shared [`Viewport`] and its semantic
//! [`Breakpoint`] classification, plus the context that exposes them to UI
//! components across the AppThere suite.
//!
//! # Single source of truth
//!
//! There is exactly one width source: the measured scroll-container width, held
//! in a `Signal<Viewport>` provided at the application root. Components never
//! re-measure — they read the *derived* [`Breakpoint`] via [`use_breakpoint`],
//! which only re-renders them when the size *class* changes, not on every pixel.
//!
//! # Wiring
//!
//! At the app root, call [`use_provide_responsive`] and feed the measured width
//! into the returned signal whenever it changes:
//! ```rust,ignore
//! let mut responsive = use_provide_responsive();
//! // …in the scroll/resize handler, from the one measured width:
//! responsive.set(Viewport::new(measured_client_width_px));
//! ```
//! Any descendant then reads:
//! ```rust,ignore
//! let bp = use_breakpoint();
//! if bp.is_compact() { /* stacked, touch-first chrome */ }
//! ```

mod breakpoint;
mod page_fit;
mod ribbon_collapse;
mod size_sensor;
mod viewport;
mod width_sensor;

pub use breakpoint::Breakpoint;
pub use page_fit::{page_fits, required_page_width, resolve_page_fit, PageFit};
pub use ribbon_collapse::{
    estimate_group_metrics, group_layout, resolve_cascade, GroupCollapse, GroupLayout,
    GroupMetrics, RibbonCascade,
};
pub use size_sensor::AtWindowSizeSensor;
pub use viewport::{Viewport, DEFAULT_DPI};
pub use width_sensor::AtViewportWidthSensor;

use dioxus::prelude::*;

/// Responsive context injected at the application root: the live measured
/// [`Viewport`] and its memoised [`Breakpoint`].
///
/// The `breakpoint` memo only recomputes (and only wakes its readers) when the
/// derived size *class* changes, so per-pixel width updates do not churn the UI.
#[derive(Clone, Copy, PartialEq)]
pub struct AtResponsiveContext {
    /// The live measured viewport (width + zoom + DPI).
    pub viewport: Signal<Viewport>,
    /// The size class derived from `viewport`, memoised on the class boundary.
    pub breakpoint: Memo<Breakpoint>,
}

/// Provides the [`AtResponsiveContext`] at the application root and returns the
/// backing [`Viewport`] signal so the app can push measured widths into it.
///
/// Seeds with [`Viewport::default`] (unmeasured → [`Breakpoint::Compact`]); the
/// first measured frame corrects it. Call once, in the root component.
pub fn use_provide_responsive() -> Signal<Viewport> {
    let viewport = use_signal(Viewport::default);
    let breakpoint = use_memo(move || viewport.read().breakpoint());
    provide_context(AtResponsiveContext {
        viewport,
        breakpoint,
    });
    viewport
}

/// Reads the [`AtResponsiveContext`] injected at the application root.
///
/// # Panics
///
/// Panics if [`use_provide_responsive`] was not called in an ancestor.
#[must_use]
pub fn use_responsive() -> AtResponsiveContext {
    use_context::<AtResponsiveContext>()
}

/// Reads the current measured [`Viewport`] from context (reactive).
#[must_use]
pub fn use_viewport() -> Viewport {
    *use_responsive().viewport.read()
}

/// Reads the current [`Breakpoint`] from context (reactive on the class
/// boundary only — the common case for responsive components).
///
/// **Resilient:** shell components are shared across the suite, but only apps
/// that called [`use_provide_responsive`] expose the context. Without it this
/// returns [`Breakpoint::Expanded`] (full chrome) rather than panicking, so an
/// app that has not wired the responsive context yet keeps its existing,
/// non-adaptive layout.
#[must_use]
pub fn use_breakpoint() -> Breakpoint {
    match try_consume_context::<AtResponsiveContext>() {
        Some(ctx) => *ctx.breakpoint.read(),
        None => Breakpoint::Expanded,
    }
}

/// Resolves the width-driven ribbon collapse cascade (Spec 04 M3 §7) for the
/// given per-group `metrics`, reactively against the measured viewport width and
/// hysteretically (the previously-resolved level is retained across resizes to
/// avoid thrash — see [`resolve_cascade`]).
///
/// **Resilient:** like [`use_breakpoint`], if no responsive context is present
/// (an app that has not wired [`use_provide_responsive`]) the available width
/// is treated as unbounded, so every group stays [`GroupCollapse::Full`] — a
/// sane full-chrome ribbon rather than a broken one. (All three suite apps now
/// wire the context; Presentation/Spreadsheet measure via
/// [`AtViewportWidthSensor`].)
///
/// The hysteresis state (the resolved collapse `level`) lives in a hook-local
/// signal, so call this once per ribbon content strip.
#[must_use]
pub fn use_ribbon_cascade(metrics: Vec<GroupMetrics>) -> RibbonCascade {
    let ctx = try_consume_context::<AtResponsiveContext>();
    // No context → unbounded width → nothing collapses (full-chrome default).
    let read_width = move || ctx.map_or(f32::MAX, |c| c.viewport.read().inner_width_px);
    let mut level = use_signal(|| 0usize);

    // Advance the hysteretic level whenever the measured width changes. Reading
    // the viewport inside the effect subscribes it to width updates.
    {
        let metrics = metrics.clone();
        use_effect(move || {
            let prev = *level.peek();
            let next = resolve_cascade(&metrics, read_width(), prev).level;
            if next != prev {
                level.set(next);
            }
        });
    }

    // Build the returned cascade from the settled level and the current width;
    // resolution is idempotent at a fixed width, so this agrees with the effect.
    let settled = *level.read();
    resolve_cascade(&metrics, read_width(), settled)
}
