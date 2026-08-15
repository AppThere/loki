// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Wiring the residency capability bound to the live zoom (Spec 08 T5.4
//! requirement 1).
//!
//! # The bound is *called*, never re-derived
//!
//! `appthere_canvas::residency::max_servable_zoom_permille` searches the real
//! planner for the highest zoom whose plan stays under the OOM-calibrated
//! ceiling. Phase 2 wrote it, tested it, and shipped it with **no caller** — so
//! `clamping_to_the_servable_zoom_makes_the_oom_branch_unreachable` guarded a
//! function nothing called, and stayed green while production could exceed the
//! ceiling it describes. That is L08-029's two-derivations failure with one of
//! the derivations missing entirely.
//!
//! So this module contains no arithmetic on the bound. It picks the page to ask
//! about, asks, and hands the answer to
//! [`DocPageSource::set_capability_limit_permille`]. Anything cleverer here would
//! be the second derivation.
//!
//! # Which page to ask about
//!
//! The bound depends on page **area**, and a document may hold several sizes
//! (Spec 08 T6.1). Asking about the largest is the only choice that is safe under
//! scrolling: a limit derived from the page currently on screen would rise as the
//! reader scrolled onto a smaller page and fall again on the way back, so the
//! effective zoom would move while nobody touched the zoom control. The largest
//! page is a property of the document, so the limit is stable for as long as the
//! document is.
//!
//! It is deliberately conservative — a Letter-only reader in a document with one
//! A2 insert is held to the A2 limit. The alternative trades a bounded loss of
//! sharpness for a zoom that changes under a scroll, which requirement 3 rejects
//! for the same reason it rejects reducing a zoom already in effect.

use appthere_canvas::residency::{PageBox, TextureBudget, max_servable_zoom_permille};

/// The largest page in a document, by area, as the residency planner's
/// [`PageBox`].
///
/// `None` for a document with no pages — a layout that has not resolved yet —
/// which the caller must treat as "no limit known", **not** as "no limit". See
/// [`capability_limit_permille`].
#[must_use]
pub fn largest_page(pages_pt: &[(f64, f64)]) -> Option<PageBox> {
    pages_pt
        .iter()
        .filter(|(w, h)| *w > 0.0 && *h > 0.0)
        .max_by(|a, b| {
            (a.0 * a.1)
                .partial_cmp(&(b.0 * b.1))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(w, h)| PageBox::new(*w, *h))
}

/// The capability limit to apply for this document, or `None` to leave the zoom
/// uncapped.
///
/// # `None` means "do not cap", and that is the safe answer here
///
/// The only way to reach `None` is a document whose layout has not produced a
/// page yet. Capping on no information would clamp every document to the floor
/// for the frame before its layout resolves — a visible zoom-out on open,
/// arriving from a state that carries no evidence about the device at all. A
/// frame at the requested zoom before the real limit lands is the cheaper error,
/// because the budget's own rasterisation scale is still in force for that frame:
/// the ceiling this bound protects is approached, not crossed, in one frame.
#[must_use]
pub fn capability_limit_permille(
    pages_pt: &[(f64, f64)],
    device_scale_factor: f64,
    budget: TextureBudget,
) -> Option<u16> {
    CapabilityInputs::from_pages(pages_pt, device_scale_factor, budget).limit()
}

/// Everything the capability bound is a function of.
///
/// Named as a type so the memo in [`crate::doc_page_source::DocPageSource::apply_capability_limit`]
/// cannot drift from the call: adding a fourth input to `max_servable_zoom_permille`
/// would be a compile error here rather than a memo that quietly stops noticing
/// it changed.
///
/// The page is `Option` because "the layout has not produced a page yet" is a
/// real state with its own answer (`None` — do not cap), and it must be part of
/// the key: a document whose first layout lands between two frames changes the
/// answer without changing the scale or the budget.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CapabilityInputs {
    /// The largest page in the document, or `None` before a layout resolves.
    pub largest_page: Option<PageBox>,
    /// The display's device pixel ratio.
    pub device_scale_factor: f64,
    /// The resident-texture budget in force.
    pub budget: TextureBudget,
}

impl CapabilityInputs {
    /// Reduces a document's page list to the inputs the bound depends on.
    ///
    /// The reduction happens **here**, before any memo key is formed: keying on
    /// the page list would recompute whenever a page the answer does not depend
    /// on changed size.
    #[must_use]
    pub fn from_pages(
        pages_pt: &[(f64, f64)],
        device_scale_factor: f64,
        budget: TextureBudget,
    ) -> Self {
        Self {
            largest_page: largest_page(pages_pt),
            device_scale_factor,
            budget,
        }
    }

    /// The bound, or `None` for a document with no resolved page.
    ///
    /// **The only place the search is called from.** Both the status bar's
    /// direct query and the render path's memo go through here, so there is one
    /// statement of what the bound is a function of — which is what lets the
    /// memo key be trusted: a fourth input would land in this struct and the
    /// key would carry it by construction.
    #[must_use]
    pub fn limit(self) -> Option<u16> {
        let page = self.largest_page?;
        Some(max_servable_zoom_permille(
            page,
            self.device_scale_factor,
            self.budget,
        ))
    }
}

/// Computes the bound for `source`'s current layout and applies it.
///
/// The one call `DocumentView` makes. Bundled so the *order* — page sizes read
/// from the live generation, limit applied, effective zoom read back — lives
/// next to the reasoning for it rather than inline in a render body where a
/// later edit can slide a line past the read-back.
/// Gated with [`crate::tile_plan`], which it reads page sizes from. The pure
/// half of this module — [`largest_page`] and [`capability_limit_permille`] —
/// stays available everywhere, because `loki-text`'s status bar calls it on
/// every platform.
#[cfg(not(any(
    all(target_os = "android", not(android_gpu)),
    all(target_os = "ios", target_abi = "sim"),
)))]
pub fn apply_to(
    source: &std::sync::Arc<crate::doc_page_source::DocPageSource>,
    device_scale_factor: f64,
    budget: TextureBudget,
) {
    let pages_pt = crate::tile_plan::page_sizes_pt(source);
    let inputs = CapabilityInputs::from_pages(&pages_pt, device_scale_factor, budget);
    // **Guarded, not unconditional.** `resolve` runs on every render, including
    // every scroll frame, and the search behind `limit()` is up to 3040
    // `plan_residency` calls — see `DocPageSource::apply_capability_limit` for
    // why re-running it on an unchanged document is pure waste.
    source.apply_capability_limit(inputs, || inputs.limit());
}

#[cfg(test)]
#[path = "zoom_capability_tests.rs"]
mod tests;
