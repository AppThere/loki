// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! How an open popover reacts to its anchor or viewport moving (Spec 08 T4.1).
//!
//! Split from `interaction.rs` at the 300-line ceiling, on a natural seam: the
//! keyboard and focus tables are about a *user action*, this is about the world
//! changing underneath one.
//!
//! # This must be driven per frame, not by scroll events
//!
//! Stated here because two instruments below depend on it and would silently
//! measure something else otherwise (L08-042).
//!
//! **Events are the wrong driver.** An anchor's viewport position changes when
//! *any* ancestor scrolls, so listening on the anchor's own container misses an
//! outer scroll — and the Recent Documents list sits inside a page that may
//! scroll too. Subscribing to every ancestor is fragile and goes stale whenever
//! the tree changes, which is exactly when it matters.
//!
//! **A per-frame rect comparison catches every cause with no plumbing at all**:
//! scroll at any depth, resize, layout shift, animation, a font finishing
//! loading. That is why the comparison is against a whole `PlacementRequest`
//! rather than a scroll delta.
//!
//! **And the counters assume it.** [`REPOSITION_BURST_WARN`] counts *consecutive*
//! repositions because a settled popover emits `Ignore` on nearly every frame —
//! a statement about frames. Driven by events, "consecutive" would count
//! consecutive *scroll events*, which a slow drag produces indefinitely without
//! anything being wrong, and the threshold would be arbitrary rather than half a
//! second.
//!
//! The frame source already exists: the scroll animator's worker-thread tick
//! (`crate::scroll::animate`).

use std::sync::atomic::{AtomicU64, Ordering};

use super::super::geometry::{place, Placement, PlacementRequest, Rect};

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

/// Consecutive repositions with no intervening [`AnchorResponse::Ignore`].
///
/// This is the loop's *signature*, and it needs no clock: a settled popover
/// emits `Ignore` on nearly every frame, while jitter emits `Reposition` on
/// every frame. Counting a burst is therefore both cheaper and more specific
/// than a rate over a window.
static CONSECUTIVE: AtomicU64 = AtomicU64::new(0);

/// Consecutive repositions after which the loop is reported.
///
/// Half a second at 60 Hz. Long enough that a genuine burst — a momentum scroll
/// through a long list — passes without comment, short enough that a loop is
/// named while someone is still looking at it.
pub const REPOSITION_BURST_WARN: u64 = 30;

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

/// Whether an anchor is still something an overlay can be attached to — i.e.
/// whether it touches the usable viewport at all.
///
/// # This exists because two modules were judging the same thing from opposite
/// ends
///
/// [`on_anchor_change`] dismisses when the anchor is not visible;
/// [`super::place`] clamps an overlay into the viewport when the anchor is
/// partly (or wholly) outside it. Neither stated where the boundary was, and a
/// caller was free to answer "visible" any way it liked — so there was a band
/// nobody owned: an anchor mostly off-screen that the caller still called
/// visible would keep a popover alive, pinned to a viewport edge, pointing at
/// something the user cannot see. Partial overlap is not exotic; it is what
/// every scroll passes through.
///
/// The band is closed by taking the viewport half of the judgement away from the
/// caller (L08-043): `on_anchor_change` computes it here, and its `bool`
/// parameter now covers **only** what geometry cannot see.
///
/// # The boundary, chosen and stated
///
/// **Any intersection keeps the popover; an empty one dismisses it.** The
/// alternatives were a fraction ("dismiss below 50% visible", a magic number
/// with no consumer behind it) and whole-containment (which dismisses the
/// instant a scroll clips one pixel of a row — the flat rule this module exists
/// to avoid). With this boundary, `place`'s main-axis clamp is exactly a floor
/// under a one-frame transient, which is what its comment claims and could not
/// previously rely on.
///
/// # Inclusive edges, because the zeroth consumer's anchor has zero width
///
/// A caret is a zero-width rect ([`PlacementRequest::anchor`] says so), so a
/// strict `<` test would report every caret sitting exactly on a viewport edge
/// as un-anchorable and dismiss the spelling menu on the left margin. Touching
/// counts.
#[must_use]
pub fn anchor_is_anchorable(anchor: Rect, viewport: Rect) -> bool {
    anchor.intersects_closed(viewport)
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
/// # Who decides "no longer visible", and where the boundary is
///
/// Two things can hide an anchor and only one of them is geometry. The viewport
/// half is decided here by [`anchor_is_anchorable`] — the caller cannot get it
/// wrong because it is no longer asked. `still_in_container` carries the half
/// this module genuinely cannot see: a row scrolled out of an inner list, a
/// collapsed section, a `display: none`. Its rect can be perfectly inside the
/// viewport while the element is not on screen at all.
///
/// Splitting it this way is what stops [`super::place`] and this function from
/// disagreeing about a partly-visible anchor — see [`anchor_is_anchorable`] for
/// the band that existed while the whole judgement was a caller's `bool`.
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
    still_in_container: bool,
) -> AnchorResponse {
    if !still_in_container || !anchor_is_anchorable(current.anchor, current.viewport) {
        return AnchorResponse::Dismiss;
    }
    // Both halves compared, and both in viewport coordinates: the anchor may
    // have moved under a still viewport (a list scrolling), or the viewport may
    // have moved under a still anchor (a window resize). Either invalidates the
    // flip decision, and comparing only one of them is how the two drift.
    if previous.anchor == current.anchor && previous.viewport == current.viewport {
        CONSECUTIVE.store(0, Ordering::Relaxed);
        return AnchorResponse::Ignore;
    }
    // Counted here rather than at the call site: this is the only place a
    // reposition is decided, so a consumer cannot forget to count one.
    REPOSITIONS.fetch_add(1, Ordering::Relaxed);
    // The counter's production voice. Emitted at the threshold only, so a
    // sustained loop reports once rather than every frame — and reports at all,
    // which a test-only assertion cannot: the cause is sub-pixel layout jitter
    // on real re-renders, which is precisely what tests do not produce. Same
    // correction as logging `reduced_tiles` unconditionally rather than only
    // under pressure (L9-011).
    if CONSECUTIVE.fetch_add(1, Ordering::Relaxed) + 1 == REPOSITION_BURST_WARN {
        tracing::warn!(
            consecutive = REPOSITION_BURST_WARN,
            ?previous.anchor,
            ?current.anchor,
            "popover repositioned on every frame — the anchor rect is changing \
             when nothing moved, which is layout jitter rather than a scroll",
        );
    }
    AnchorResponse::Reposition(place(current))
}
