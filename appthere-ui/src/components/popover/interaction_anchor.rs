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
//! # There is no frame source, and the driver is therefore event-driven (r72)
//!
//! **Retracted:** this section used to end "the frame source already exists: the
//! scroll animator's worker-thread tick". It does not. `scroll::animate` spawns
//! **one thread per animation**, runs `(180 / 16) + 2` = 13 ticks and exits — an
//! *animation clock*, live only during the three smooth-scroll gestures T1.3
//! names. Wired to it, this module would compare rects only during a Find or a
//! Go To Page and never for a wheel scroll, a drag, a resize or a reflow — and
//! `idle_frames_perform_no_repositions` would pass **because no frames run at
//! all**, which is worse than a wrong threshold.
//!
//! Nothing else in this stack ticks. What exists are *change* sources:
//! `ScrollMetrics` (written from `onscroll`, and it fires for every scroll
//! whatever caused it) and `AtWindowSizeSensor`. So the driver is event-driven,
//! and the section above is right that this changes what the counter means.
//!
//! **The counter's subject changes from a rate to a multiplicity, and that is
//! the honest restatement rather than a re-tuned threshold.** Under an event
//! driver every comparison has a cause, and every cause is a real one — so a
//! long run of repositions during a trackpad drag is the module *working*, and a
//! threshold on consecutive repositions would fire on it. But the hazard the
//! counter exists for does not disappear; it changes shape. A jitter loop is now
//! **reactive rather than temporal**: writing `resolved` re-renders the host,
//! which can re-run the comparison, which writes `resolved` again — so it shows
//! up as *more than one reposition attributable to a single event*, not as many
//! repositions over many frames.
//!
//! So when the driver lands: count repositions **per event**, warn above a small
//! number rather than 30, and replace `idle_frames_perform_no_repositions` with
//! *one event produces at most one reposition* — which has content under an
//! event driver, where the idle assertion would be as vacuous as it was under the
//! animation clock. [`REPOSITION_BURST_WARN`]'s "half a second at 60 Hz"
//! derivation does not survive that change and must be re-derived, not rescaled.
//!
//! # A shrinking viewport repositions; it does not close the menu (r68)
//!
//! **Stated as a decision because it changed, and because the reasoning is not
//! recoverable from the code.** The clamp in `present` makes this fall out
//! mechanically, so a later reader will find the behaviour surprising and find
//! nothing explaining it.
//!
//! The rule used to be *a change of form dismisses*. When `present` could return
//! `Presentation::Modal`, plumbing that through `Reposition` would have let an
//! open menu **become a full-screen sheet under the user's hands** — startling,
//! and with no focus story, since the anchored form's focus lives in a list and
//! the modal's does not. Dismissal was the other defensible answer and the one
//! taken.
//!
//! That argument was sound and its premise is gone. There is no second form:
//! `present` clamps an overlay too small to use up to the usable floor and
//! returns a `Placement`, because nothing ever implemented the modal one and its
//! two recipients read it incompatibly (r68, see [`super::presentation`]). With
//! one form there is no form change, so the only question left is what a
//! *shrinking* viewport should do — and dismissal was never argued for on its
//! own merits, only as the lesser of two bad transitions.
//!
//! **Repositioning is right on the merits.** The cause is rotation or a
//! multi-window resize; the soft keyboard cannot arrive here, because while a
//! popover is open focus is *inside* it, so no field is gaining focus and no IME
//! is being raised (a viewport already shortened by the keyboard is an open-time
//! condition `present` handles at open). Rotation is deliberate — but it is
//! deliberate about the *window*, not about the menu. Someone rotating a phone
//! to read a menu more comfortably is served badly by the menu closing, and
//! nothing else in this shell closes on rotation. The overlay stays where the
//! user put it, at the smallest size it can still be operated at, and scrolls.
//!
//! [`super::DismissCause::PresentationChanged`] survives for the focus table
//! and now has no producer here; it is kept rather than deleted because a
//! consumer that implements a real modal fallback (T5.2's colour picker is the
//! likely first) will need exactly that cause.
//!
#[path = "interaction_anchor_counter.rs"]
mod counter;
pub use counter::{repositions, reset_repositions, REPOSITION_BURST_WARN};

use counter::{CONSECUTIVE, REPOSITIONS};
use std::sync::atomic::Ordering;

use super::super::geometry::{Placement, PlacementRequest, Rect};
use super::super::presentation::present;

/// What a scroll or resize does to an open popover.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AnchorResponse {
    /// Nothing moved that matters.
    Ignore,
    /// Re-place the popover. **Carries the recomputed placement** — see
    /// [`on_anchor_change`] for why it is not a bare marker.
    ///
    /// A `Placement` rather than a `Presentation` because of the invariant
    /// below: a reposition is always *still anchored*, and always at a size the
    /// consumer said was usable. A change of form does not arrive here — it
    /// arrives as `Dismiss`.
    Reposition(Placement),
    /// Close: the anchor is no longer visible, or the presentation would have to
    /// change form — see [`DismissCause`](super::DismissCause).
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
    // **The form-change rule is retired with the modal form itself (r68).**
    //
    // This used to `match present(current)` and map a `Modal` outcome to
    // `Dismiss`. An earlier draft compared the previous form against the current
    // one; a mutation removing that comparison broke no test, and the surviving
    // mutation was the finding rather than a gap in the suite — an open popover
    // is anchored by definition, so "the form changed" and "the current geometry
    // is not anchored" were the same condition. Unreachable by subsumption, the
    // same shape the M4 mutation found in `place`.
    //
    // The modal arm is now gone for a different reason: nothing implemented it,
    // and this site read it as *dismiss* while the host read it as *suppress*.
    // `present` clamps to the usable floor instead, so a popover no longer
    // vanishes because the window got short.
    AnchorResponse::Reposition(present(current))
}
