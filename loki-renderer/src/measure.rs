// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The active reading **measure cap** for the reflow view (Spec 08 T7.2).
//!
//! Like [`crate::spell`] and [`crate::revision`], this is app-internal ambient
//! state rather than a parameter — and here that is not merely convenient, it is
//! the safe shape.
//!
//! # Why ambient, and not an argument
//!
//! `reflow_layout_content_width_pt` is the **single source** of reflow width
//! (Spec 01 A-1): paint, hit-testing, caret placement and keyboard navigation
//! each call it, in two crates, and a layout built at a width the paint did not
//! use puts the caret in the wrong place. Adding a parameter would give four
//! call sites the opportunity to pass four values, and three of them would be
//! discovered as a mis-placed caret rather than as a compile error.
//!
//! So the cap is installed once, by whoever resolved it, and every call site
//! keeps the signature it has. That is rule 5 the other way round: the wrong
//! thing is not documented, it is unavailable.
//!
//! # Why the cap is a width when the setting is a character count
//!
//! Resolving a character count needs `FontResources` — a `&mut` handle held
//! behind a mutex on the layout path — and the reflow width is asked for during
//! hit-testing and painting, where taking that lock would be both a contention
//! hazard and a lock-ordering one. So the *resolution* happens where the fonts
//! already are ([`loki_layout::measure::measure_width_pt`]) and the *result* is
//! what is published here.
//!
//! The cost is that the cap goes stale if the body font changes and nobody
//! re-resolves. `TODO(measure-refresh)`: re-resolve when the document's body
//! style changes, which is the event that moves it.

use std::sync::{PoisonError, RwLock};

use crate::render_layout::{MAX_REFLOW_TILE_PX, PX_TO_PT, REFLOW_PADDING_PT};

/// The active measure cap: the widest reflow **content** column, in points.
///
/// `None` until something resolves one, which is what keeps the pre-T7.2
/// behaviour — the fixed [`MAX_REFLOW_TILE_PX`] tile — as the fallback rather
/// than a guess. An app that never installs a measure renders exactly as it did.
static CONTENT_CAP_PT: RwLock<Option<f32>> = RwLock::new(None);

/// Installs the resolved reading measure as a content-width cap, in points.
///
/// A non-finite or non-positive value clears the cap instead of storing it: a
/// bad measurement must fall back to the constant, not produce a zero-width
/// reading column that renders nothing and looks like the document failed to
/// open.
pub fn set_content_cap_pt(cap_pt: Option<f32>) {
    let sane = cap_pt.filter(|c| c.is_finite() && *c > 0.0);
    *CONTENT_CAP_PT
        .write()
        .unwrap_or_else(PoisonError::into_inner) = sane;
}

/// The active content cap in points, or `None` when none is installed.
#[must_use]
pub fn content_cap_pt() -> Option<f32> {
    *CONTENT_CAP_PT
        .read()
        .unwrap_or_else(PoisonError::into_inner)
}

/// The widest reflow **tile** in CSS px, given the active measure.
///
/// The tile is the content column plus its two [`REFLOW_PADDING_PT`] insets, so
/// the cap is converted at the same boundary the rest of the reflow metrics use
/// — and it is only ever *narrower* than [`MAX_REFLOW_TILE_PX`], never wider.
///
/// # Why the constant survives as a ceiling
///
/// A measure resolved against a very large body size would ask for a column
/// wider than the old fixed cap, and widening the reading column is not
/// something this task asked for: T7.2 replaces *how the cap is chosen*, not the
/// promise that the reflow view holds a comfortable line length. Taking the
/// minimum keeps the change one-directional, so a bad measurement can only make
/// lines shorter — the harmless failure — and never longer.
#[must_use]
pub fn max_tile_width_px() -> f32 {
    match content_cap_pt() {
        Some(content_pt) => {
            let tile_px = (content_pt + 2.0 * REFLOW_PADDING_PT) / PX_TO_PT;
            tile_px.min(MAX_REFLOW_TILE_PX)
        }
        None => MAX_REFLOW_TILE_PX,
    }
}

#[cfg(test)]
#[path = "measure_tests.rs"]
mod tests;
