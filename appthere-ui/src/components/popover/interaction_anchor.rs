// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! How an open popover reacts to its anchor or viewport moving (Spec 08 T4.1).
//!
//! Split from `interaction.rs` at the 300-line ceiling, on a natural seam: the
//! keyboard and focus tables are about a *user action*, this is about the world
//! changing underneath one.

use std::sync::atomic::{AtomicU64, Ordering};

use super::super::geometry::{place, Placement, PlacementRequest};

/// Repositions performed since the last [`reset_repositions`].
///
/// # Why a counter and not an epsilon
///
/// [`on_anchor_change`] compares anchor and viewport rects for **exact float
/// equality**. That is the right comparison — an epsilon would trade a jitter
/// loop for staleness, and staleness is the defect this whole module exists to
/// prevent. But the inputs come from layout, so if a re-render ever produces a
/// sub-pixel-different rect for an unmoved anchor, the popover re-places every
/// frame: the same predicate hazard that moved zoom to integer permille, in a
/// place where an integer representation is not available.
///
/// A loop like that presents as a vague frame-rate symptom and sends you looking
/// at rendering. Counting instead puts it in the register that has been reliable
/// — four retracted timing attributions, no wrong counter — and turns it into an
/// immediate, nameable failure: **repositions must not advance across idle
/// frames.**
static REPOSITIONS: AtomicU64 = AtomicU64::new(0);

/// Repositions since the last reset. Assert this is unchanged across frames in
/// which nothing moved.
#[must_use]
pub fn repositions() -> u64 {
    REPOSITIONS.load(Ordering::Relaxed)
}

/// Resets the reposition counter.
pub fn reset_repositions() {
    REPOSITIONS.store(0, Ordering::Relaxed);
}

/// What a scroll or resize does to an open popover.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AnchorResponse {
    /// Nothing moved that matters.
    Ignore,
    /// Re-place the popover. **Carries the recomputed placement** — see
    /// [`on_anchor_change`] for why it is not a bare marker.
    Reposition(Placement),
    /// Close: the anchor is no longer visible.
    Dismiss,
}

/// How an open popover responds when its anchor's viewport rect may have moved.
///
/// # "Dismiss on scroll" is too blunt, and "reposition" is easy to get wrong
///
/// T4.2's Recent Documents menu anchors to an entry *inside a scrolling list*.
/// Under a flat dismiss-on-scroll rule, nudging a trackpad closes the menu — and
/// the list is the thing you scroll to reach entries. A flat rule also errs the
/// other way: an unrelated pane scrolling should not close a menu being read.
///
/// So the decision keys on the anchor's rect **in viewport coordinates**:
///
/// | condition | response |
/// | --- | --- |
/// | no longer visible | `Dismiss` — anchoring to something off-screen is meaningless |
/// | rect changed | `Reposition` |
/// | rect unchanged | `Ignore` — covers the unrelated-pane scroll for free |
///
/// # Viewport coordinates, not container coordinates
///
/// An earlier draft asked "is the anchor still visible *in its container*", and
/// that predicate is wrong in a way this component exists to prevent. A Recent
/// Documents entry can sit unmoved and fully visible inside its list while the
/// *list* scrolls in the page, or while the window resizes. Container-visibility
/// stays true, so the popover holds its position — and the flip decision it was
/// placed with has gone stale. A menu that opened downward with room below now
/// hangs off the bottom: **the exact defect T4.1 exists to fix, arriving by a
/// path the geometry cannot see.**
///
/// The caller passes the request it **last placed against** and the request that
/// is **true now**; the difference between them is the whole input. That is one
/// comparison rather than a scroll-delta plus a resize hook, so there is no
/// second path for the two to diverge along (L08-028).
///
/// # Why `Reposition` carries a `Placement`
///
/// Because the tempting implementation is to offset the popover by the scroll
/// delta, and that is wrong for the same reason: an offset preserves a flip
/// decision made against different viewport bounds. Re-placement has to re-run
/// [`super::place`] against the *current* anchor and the *current* viewport.
///
/// Handing back the recomputed `Placement` rather than a bare `Reposition`
/// marker makes the delta shortcut unavailable — there is nothing for a caller
/// to offset, only a position to adopt. Same move as deriving the focus trap
/// from the role: the wrong state is not expressible.
///
/// **Window resize routes here too**, deliberately. It is the same class —
/// anchor unmoved in its container, viewport bounds changed — and giving it its
/// own path is how the two drift.
#[must_use]
pub fn on_anchor_change(
    previous: PlacementRequest,
    current: PlacementRequest,
    anchor_visible: bool,
) -> AnchorResponse {
    if !anchor_visible {
        return AnchorResponse::Dismiss;
    }
    // Both halves compared, and both in viewport coordinates: the anchor may
    // have moved under a still viewport (a list scrolling), or the viewport may
    // have moved under a still anchor (a window resize). Either invalidates the
    // flip decision, and comparing only one of them is how the two drift.
    if previous.anchor == current.anchor && previous.viewport == current.viewport {
        return AnchorResponse::Ignore;
    }
    // Counted here rather than at the call site: this is the only place a
    // reposition is decided, so a consumer cannot forget to count one.
    REPOSITIONS.fetch_add(1, Ordering::Relaxed);
    AnchorResponse::Reposition(place(current))
}
