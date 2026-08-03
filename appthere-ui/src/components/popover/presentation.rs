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
//! # The first answer was a modal fallback, and it is withdrawn (r68)
//!
//! r48 borrowed T7.4's decision — when the anchored form will not fit, present
//! modally — and returned a `Presentation::Modal` for the caller to honour.
//! **No caller ever did.** Worse, the two that received it disagreed: the host
//! rendered nothing and `interaction::on_anchor_change` dismissed. One outcome,
//! two incompatible readings, in the module whose invariant is that a decision
//! is made in exactly one place.
//!
//! So the floor is now applied by **clamping**: an overlay too small to use is
//! grown to the minimum and scrolls its content. A real modal fallback should
//! land with the consumer that wants one — T5.2's colour picker is the likely
//! first — because that is the only way it acquires an implementation instead of
//! a second interpretation.
//!
//! # Deleted here, kept at `DismissCause::PresentationChanged` — the same rule
//!
//! Those two rulings look opposite and are not. **Reachable-but-unimplemented is
//! a defect; unreachable-and-marked is a deferral.** `Modal` was produced and
//! read two incompatible ways, so it was a live inconsistency and had to become
//! unrepresentable. [`super::interaction::DismissCause::PresentationChanged`] is
//! produced by nothing, so nothing can disagree about it — it parks a settled
//! focus decision at the cost of one test, for the consumer that will need it.
//!
//! Dismissing the soft keyboard to reclaim its ~180px was the other candidate.
//! It reads well for the keyboard case specifically and does not generalise: a
//! short window with no keyboard up has nothing to reclaim.
//!
//! # The threshold is a house standard, not a taste
//!
//! [`MIN_ANCHORED_HEIGHT_PX`] is `TOUCH_MIN` — 44px, WCAG 2.5.8, which CLAUDE.md
//! already requires every interactive component to meet. An anchored menu
//! shorter than that cannot present **one** legal touch target, so it is grown
//! rather than shown cramped. Picking any other number would have been a taste
//! with no consumer behind it.
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

use super::geometry::{place, Placement, PlacementRequest, Rect, Side};
use crate::tokens::spacing::TOUCH_MIN;

/// Absolute floor for an anchored overlay: one WCAG 2.5.8 touch target.
///
/// Below this the anchored form cannot present a single actionable row at the
/// minimum size the house standard requires, so it is not a small menu — it is
/// an unusable one. Applied by [`present`] whatever a consumer asks for, so a
/// request of `0.0` still cannot produce a sub-target menu.
///
/// **This is a floor, not the decision point.** It bounds *unusable*; it does
/// not mark where the modal becomes the better presentation — see
/// [`MIN_ANCHORED_MENU_PX`].
pub const MIN_ANCHORED_HEIGHT_PX: f32 = TOUCH_MIN;

/// The height of one menu row.
///
/// # Equal to the touch minimum *today*, and that is a coincidence worth naming
///
/// A menu row is sized by its touch target while every dimension in this crate
/// is device-fixed. Once I-24 lands and row heights become text-relative, a row
/// at 2.0× text is **taller than a touch target** — and
/// [`MIN_ANCHORED_MENU_PX`], written as `2 × TOUCH_MIN`, would silently stop
/// meaning *two rows*. The popover would then fall back to modal later than
/// intended and show two **clipped** rows instead of two whole ones: the
/// threshold still passing its own test while no longer bounding what it was
/// derived to bound.
///
/// Naming the term now makes that a one-line change at the right knob instead of
/// a rediscovery. When rows scale, this becomes the scaling quantity and the
/// derivation below stays correct by construction — which is the difference
/// between a constant that ages and one that misleads (L08-031).
pub const MENU_ROW_HEIGHT_PX: f32 = TOUCH_MIN;

/// What a **menu** should pass as its minimum anchored height: two rows.
///
/// # Why two, and why it is not inherited from the floor
///
/// The floor above was chosen to bound "unusable" — one touch target — and one
/// row is a poor place to *stay anchored*: a 44px window scrolling a list of
/// four actions shows one item with **no indication that the others exist**. The
/// second row is the affordance. A list that visibly continues reads as a list;
/// a single row reads as the whole menu, so a user does not scroll and never
/// learns what was there.
///
/// Two also names the point where the alternative is plainly better rather than
/// merely different: at two rows a modal shows the same four actions at once,
/// with no scrolling and no ambiguity. Since the fallback exists and is good,
/// the threshold should sit where it wins, not at the last pixel where the
/// anchored form is technically legal.
///
/// Expressed in [`MENU_ROW_HEIGHT_PX`] rather than in touch targets, because
/// **two rows** is the decision and two touch targets is only what that measures
/// today.
///
/// The named consumers are why one row is not enough for any of them: T4.2's
/// Recent Documents entry menu carries several actions, and T5.4's zoom popover
/// carries a preset list plus a field. **Panels are not menus** — T5.2's colour
/// picker has a content minimum of its own (an SV square has a size below which
/// it cannot be used), which is why this is a value a consumer passes rather
/// than a constant the primitive applies to everything.
pub const MIN_ANCHORED_MENU_PX: f32 = 2.0 * MENU_ROW_HEIGHT_PX;

/// Places the overlay, guaranteeing it is at least tall enough to use.
///
/// This is the entry point consumers use. [`place`] stays public because
/// repositioning and the geometry tests need the raw result, but a consumer that
/// calls it directly has to make this decision itself, which is the thing this
/// function exists to stop four of them doing four ways.
///
/// # The modal fallback is withdrawn (r68), and the reason is worse than "unused"
///
/// This returned a `Presentation` enum whose `Modal` arm meant "the anchored form
/// does not fit; show the content some other way". **No consumer ever implemented
/// it**, and the two call sites that received it disagreed about what it meant:
/// `AtPopoverContext::open_resolved` stored no placement, so the host rendered
/// *nothing*, while [`super::interaction::on_anchor_change`] mapped it to
/// `Dismiss`. Suppress versus dismiss, for one outcome, inside the module whose
/// stated invariant is that every decision is made in exactly one place. A
/// consumer reading either site would have concluded the wrong thing about the
/// other — L08-029 in the primitive built to prevent it.
///
/// A reachable outcome with no implementation is worse than a missing feature: it
/// is a **dead control** that looks handled. So the variant is gone and the floor
/// is applied by clamping instead. If T5.4's zoom popover or T5.2's colour picker
/// wants a genuine modal fallback — plausible for the picker, whose SV square,
/// hue strip and fields are tall — it lands **with** that consumer, which is the
/// only way it gets an implementation rather than a second interpretation.
///
/// # Three constraints, and the order they concede in
///
/// The floor, the anchor and the viewport can all three be unsatisfiable at
/// once, and this function has to pick which gives. The order is:
///
/// 1. **The floor.** An overlay shorter than one touch target cannot be operated
///    at all, so the height is `min` whatever else it costs.
/// 2. **Never across the anchor.** The edge touching the anchor does not move.
/// 3. **The viewport**, last — the grown overlay may hang off the edge it grew
///    towards, and `clamped` says so.
///
/// **In the viewport-bound case all three cannot hold**, and that is not a
/// corner: [`place`] measures room as the viewport less the gap and the edge
/// margin, and returns a short overlay only when the roomier side holds less
/// than the floor — so growing away from the anchor *always* breaches that edge
/// margin, and breaches the viewport itself by `floor - room - margin` whenever
/// that is positive. Conceding the viewport rather than the anchor is a choice
/// between two bad placements, made on which failure a reader can see:
///
/// - An overlay clipped by the screen edge **looks clipped**. Content is
///   missing and the user knows it.
/// - An overlay across its own trigger **looks correct**. The trigger is
///   obscured by the thing it opened, the re-click that would close it lands on
///   the menu instead, and nothing on screen says so — the silent class this
///   module already refuses once, at `Presentation::Modal`.
///
/// It also keeps the forced case *detectable*: `!placement.rect.is_inside(vp)`
/// now means "the anchored form genuinely failed here", which is the trigger
/// condition a real modal fallback needs. `clamped` cannot serve that — [`place`]
/// sets it for ordinary reduction too. Under the previous rule the forced case
/// was indistinguishable from a good placement, so nothing could ever have
/// escalated out of it.
///
/// The remaining answer for this regime is the modal, and it lands with the
/// consumer that wants one, for the reason given above.
#[must_use]
pub fn present(req: PlacementRequest) -> Placement {
    let placed = place(req);
    // The consumer's minimum, floored by the house standard: a request below one
    // touch target is not honoured, so `min_anchored_height: 0.0` is not a way
    // back to the old behaviour (L08-043).
    let min = req.min_anchored_height.max(MIN_ANCHORED_HEIGHT_PX);
    if placed.rect.height >= min {
        return placed;
    }
    // Grow **away from the anchor**, which is side-dependent: the edge touching
    // the anchor is the bottom for `Above` and the top for `Below`, and that edge
    // must not move. Growing downward regardless — which this did until the
    // branch review — puts an `Above` overlay straight across the trigger that
    // opened it, and a covered trigger cannot be clicked to close the menu.
    //
    // `place` guarantees the overlay and the anchor never overlap, and
    // `geometry_place` calls that invariant "asserted rather than argued". It was
    // argued: `an_overlay_never_covers_its_own_anchor` asserts against `place`,
    // while every consumer reaches geometry through `present` — so the check
    // covered the one path the defect could not reach. Rule 3: an instrument
    // that speaks only where the question is already settled.
    let top = match placed.side {
        Side::Above => placed.rect.bottom() - min,
        Side::Below => placed.rect.y,
    };

    // **And it is not clamped back into the viewport**, which is the second half
    // of the same decision — see the ordering note on this function.
    Placement {
        rect: Rect {
            y: top,
            height: min,
            ..placed.rect
        },
        clamped: true,
        ..placed
    }
}

#[cfg(test)]
#[path = "presentation_tests.rs"]
mod tests;
