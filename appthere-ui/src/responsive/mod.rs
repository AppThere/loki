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
mod viewport;

pub use breakpoint::Breakpoint;
pub use viewport::{Viewport, DEFAULT_DPI};

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
#[must_use]
pub fn use_breakpoint() -> Breakpoint {
    *use_responsive().breakpoint.read()
}
