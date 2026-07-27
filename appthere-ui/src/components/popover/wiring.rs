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

/// A stable identifier for whatever the anchor *is* — a document id, a command
/// id — as opposed to where it sits.
///
/// # Derive it from content, never from position
///
/// An index into the list is not a key. **Recent Documents reorders** — opening
/// a document moves it to the top — so under an index-based key
/// `opened_for == under_anchor` holds while a *different* document sits under
/// the anchor. That is precisely the failure this type exists to catch,
/// reintroduced through a weak key, and it is worse than having no check because
/// the check now reports `Same`.
///
/// A path hash is stable under both reordering and recycling; an index is stable
/// under neither. Recycling alone would be caught by an index — which is what
/// makes the trap easy to fall into, since the virtualisation case is the one
/// the guard was written for and the reordering case is the one the list
/// actually does today.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct AnchorKey(pub u64);

/// Whether the thing under the anchor is still the thing the popover was opened
/// for.
///
/// # The failure geometry cannot see
///
/// A virtualised list recycles DOM nodes. If the row a menu is open for scrolls
/// away and a **different** row is recycled into the same node, the anchor rect
/// is unchanged — so [`super::interaction::on_anchor_change`] correctly reports
/// `Ignore`, the menu stays open, and every command in it now acts on the wrong
/// document. Silent, plausible, and destructive.
///
/// `AnchorScrolledAway` does not cover it: nothing scrolled away from the
/// popover's point of view, because the rect it watches never moved.
///
/// **The guard belongs in identity, not in geometry**, which is why it is a
/// separate question with a separate input. Retrofitting identity into a
/// component whose comparison is purely geometric would mean revisiting all four
/// consumers, so it is here before the first of them exists.
///
/// The Recent Documents list is not virtualised today, so this is a guard rather
/// than a fix — but the cost of having it now is one `u64` per open popover.
///
/// # Check this *before* the geometric comparison
///
/// A recycled row returns `Ignore` from the geometry, so a caller that asks
/// geometry first and identity second has already decided to do nothing.
#[must_use]
pub fn on_anchor_identity(opened_for: AnchorKey, under_anchor: Option<AnchorKey>) -> IdentityCheck {
    match under_anchor {
        Some(key) if key == opened_for => IdentityCheck::Same,
        // Both "nothing there" and "something else there" dismiss, and for the
        // same reason: the popover's commands have no valid target. They are
        // distinguished only so a diagnostic can say which happened.
        Some(_) => IdentityCheck::Recycled,
        None => IdentityCheck::Gone,
    }
}

/// The outcome of an anchor identity check.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IdentityCheck {
    /// Still the same target — carry on to the geometric comparison.
    Same,
    /// A different target now occupies the anchor. Dismiss.
    Recycled,
    /// Nothing occupies the anchor. Dismiss.
    Gone,
}

impl IdentityCheck {
    /// Whether the popover must close.
    #[must_use]
    pub fn must_dismiss(self) -> bool {
        !matches!(self, Self::Same)
    }
}
