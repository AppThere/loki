// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! What form the overlay takes when the anchored one will not fit (Spec 08
//! T4.1, r48).
//!
//! # A tap that produces nothing is its own defect class
//!
//! `place` can return a placement with no usable height — the anchor leaves no
//! room on either side — and the first answer to that was "the consumer checks
//! and suppresses". That leaves a **dead control**: a button that responds to a
//! tap by doing nothing, which is the same class as a menu that opens and cannot
//! be clicked or a key that routes nowhere. It is also the failure hardest to
//! report, because nothing appears to go wrong.
//!
//! # The decision already exists — T7.4
//!
//! Spec 08 decided this once: when nested scroll cannot support an in-place
//! presentation, T7.3's oversized-element viewer **falls back to a modal
//! full-screen viewer** rather than degrading. Same shape here, so the same
//! answer: when the anchored form cannot fit, present modally rather than not at
//! all.
//!
//! Reusing it beats inventing a second answer to the same question, and it gives
//! the popover **one** fallback rather than one per consumer — which is the
//! failure the "scope for four consumers" instruction exists to prevent.
//!
//! Dismissing the soft keyboard to reclaim its ~180px was the other candidate.
//! It reads well for the keyboard case specifically and does not generalise: a
//! short window with no keyboard up has nothing to reclaim.
//!
//! # The threshold is a house standard, not a taste
//!
//! [`MIN_ANCHORED_HEIGHT_PX`] is `TOUCH_MIN` — 44px, WCAG 2.5.8, which CLAUDE.md
//! already requires every interactive component to meet. An anchored menu
//! shorter than that cannot present **one** legal touch target, so the anchored
//! form is not permitted rather than merely cramped. Picking any other number
//! would have been a taste with no consumer behind it.
//!
//! # Why the trigger cannot be predicted, only handled
//!
//! The reachable case is a template tile in a short viewport, and the term that
//! grows is the **label** — which scales with the user's text-size setting.
//! `DeviceProfile` has `device_scale_factor` and `reduced_motion` but **no font
//! scale**: nothing in this workspace observes the accessibility text size, so
//! no consumer can predict whether its own anchor has crossed the line. A
//! fallback conditioned on cause would therefore be unreliable by construction;
//! this one is conditioned only on the geometry that comes out.

use super::geometry::{place, Placement, PlacementRequest};
use crate::tokens::spacing::TOUCH_MIN;

/// Shortest anchored overlay worth showing: one WCAG 2.5.8 touch target.
///
/// Below this the anchored form cannot present a single actionable row at the
/// minimum size the house standard requires, so it is not a small menu — it is
/// an unusable one.
pub const MIN_ANCHORED_HEIGHT_PX: f32 = TOUCH_MIN;

/// How the overlay should be presented.
///
/// Returned instead of a bare [`Placement`] so "render an overlay too small to
/// use" is not a state a consumer can reach by forgetting to check — the same
/// move as deriving the focus trap from the role, and as `Reposition` carrying
/// its recomputed placement (L08-043).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Presentation {
    /// Anchored to its trigger at this placement.
    Anchored(Placement),
    /// Full-screen modal: the anchored form does not fit. **The consumer must
    /// still show the content** — this is a change of form, not a suppression.
    Modal,
}

/// Places the overlay, falling back to [`Presentation::Modal`] when the anchored
/// form cannot carry a usable menu.
///
/// This is the entry point consumers use. [`place`] stays public because
/// repositioning and the geometry tests need the raw result, but a consumer that
/// calls it directly has to make this decision itself, which is the thing this
/// function exists to stop four of them doing four ways.
#[must_use]
pub fn present(req: PlacementRequest) -> Presentation {
    let placed = place(req);
    if placed.rect.height < MIN_ANCHORED_HEIGHT_PX || placed.rect.width <= 0.0 {
        return Presentation::Modal;
    }
    Presentation::Anchored(placed)
}

#[cfg(test)]
#[path = "presentation_tests.rs"]
mod tests;
