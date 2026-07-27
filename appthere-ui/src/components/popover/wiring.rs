// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Two decisions that live only in wiring, made explicit so they are not settled
//! by whichever line was typed first (Spec 08 T4.1).

use super::geometry::Rect;

/// Whether a click at `point` should dismiss a popover.
///
/// # The anchor is not outside
///
/// **The defect every popover implementation ships once:** clicking the trigger
/// while the popover is open counts as an outside click, so the popover
/// dismisses — and then the trigger's own handler fires and reopens it. The
/// control appears not to toggle, and the cause is invisible in both handlers
/// because each is individually correct.
///
/// So the outside region excludes the anchor as well as the popover. A click on
/// the trigger is then the trigger's business: it toggles, which is what a
/// trigger is for.
///
/// Rects rather than node identity because that is what the pure modules already
/// speak, and because it is testable without a tree.
#[must_use]
pub fn is_outside_dismiss(point: (f32, f32), popover: Rect, anchor: Rect) -> bool {
    !contains(popover, point) && !contains(anchor, point)
}

fn contains(r: Rect, (x, y): (f32, f32)) -> bool {
    x >= r.x && x <= r.right() && y >= r.y && y <= r.bottom()
}

/// Identifies an open popover, so "which one" has an answer.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct PopoverId(pub u64);

/// What opening a popover does to one already open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpenResponse {
    /// Nothing was open; just open this one.
    Open,
    /// Dismiss this one first — it is a different popover.
    DismissThenOpen(PopoverId),
    /// Already open. Opening it again is a no-op, not a re-entry.
    AlreadyOpen,
}

/// Decides what happens when `opening` is opened while `currently_open` is up.
///
/// # One popover at a time, and that is a decision
///
/// A list of entries each with a menu button is precisely where a user produces
/// a second one, so T4.2 would have met this immediately. Two live popovers give
/// an ambiguous reposition counter — which is a process-wide static, and so
/// *assumes* a singleton — and a focus restoration that does not know which
/// anchor to return to.
///
/// The alternative is keying both the counter and the focus target per popover,
/// which buys a capability nobody has asked for: none of the four consumers
/// wants two menus visible at once. So the singleton is enforced here, and
/// **opening a second dismisses the first** rather than the two coexisting.
///
/// Re-opening the *same* popover is [`OpenResponse::AlreadyOpen`] rather than a
/// dismiss-then-open, because the dismiss would run the focus sequence and land
/// focus on the anchor mid-open — a flicker that reads as the menu closing and
/// reopening under the pointer.
#[must_use]
pub fn open_response(currently_open: Option<PopoverId>, opening: PopoverId) -> OpenResponse {
    match currently_open {
        None => OpenResponse::Open,
        Some(id) if id == opening => OpenResponse::AlreadyOpen,
        Some(id) => OpenResponse::DismissThenOpen(id),
    }
}

#[cfg(test)]
#[path = "wiring_tests.rs"]
mod tests;
