// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The reposition counter and its burst threshold (Spec 08 T4.1).
//!
//! Split from `interaction_anchor.rs` at the 300-line ceiling (r72), when
//! settling D-15 grew the driver's reasoning. A natural seam: that module is
//! about *what a movement means*, this is the instrument watching for the one
//! hazard the comparison itself can create — and D-15's reasoning is about
//! exactly that hazard, so it lives here with the instrument rather than beside
//! the semantics.
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
//! and that changes what this counter means.
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
//! # The driver must not write an unchanged placement — and that is new (r72)
//!
//! This obligation did not exist under a frame model and does under an event
//! one, which is why it is stated rather than assumed. `Signal::set` notifies
//! unconditionally: it does not compare. So a driver that writes `resolved` on
//! every event re-renders the host on every event, and the host's re-render can
//! re-enter the comparison — a cycle that terminates only because the values
//! eventually stop differing, which is not a guarantee, it is a coincidence of
//! the input settling.
//!
//! [`super::on_anchor_change`] is already correct here: it returns
//! [`super::AnchorResponse::Ignore`]
//! when nothing moved. **The obligation is the driver's** — it must not write on
//! `Ignore`, and must compare before writing on `Reposition`, which
//! [`crate::components::popover::geometry::Placement`] supports directly (it
//! is `PartialEq`).
//!
//! This is what makes *one event produces at most one reposition* the right
//! assertion rather than a restatement of the counter: it **passes when the write
//! is idempotent and fails informatively when it is not**, which is the property
//! actually at risk. A test of the counter alone would not distinguish them.
//!

use std::sync::atomic::{AtomicU64, Ordering};

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
pub(super) static REPOSITIONS: AtomicU64 = AtomicU64::new(0);

/// Consecutive repositions with no intervening [`AnchorResponse::Ignore`].
///
/// This is the loop's *signature*, and it needs no clock: a settled popover
/// emits `Ignore` on nearly every frame, while jitter emits `Reposition` on
/// every frame. Counting a burst is therefore both cheaper and more specific
/// than a rate over a window.
pub(super) static CONSECUTIVE: AtomicU64 = AtomicU64::new(0);

/// Consecutive repositions after which the loop is reported.
///
/// Half a second at 60 Hz. Long enough that a genuine burst — a momentum scroll
/// through a long list — passes without comment, short enough that a loop is
/// named while someone is still looking at it.
///
/// **This derivation assumes a frame driver, and there is none** (see
/// `interaction_anchor`'s docs, D-15). When the event-driven driver lands, the
/// unit becomes repositions *per event* and this number must be **re-derived
/// rather than rescaled**: 30 is half a second of frames, and there is no
/// defensible way to read that as a count of events. Left as-is because nothing
/// drives the counter yet, and a number changed ahead of its consumer is one
/// nobody can check.
///
/// **The name goes with it, in the same change and not after.** Under an event
/// driver "burst" and "consecutive" describe an ordinary trackpad drag — 30
/// consecutive events is a person scrolling — so the identifier would assert a
/// property it does not have, which this program treats as a defect rather than
/// a wart (L08-031). If this constant survives to the driver landing, renaming it
/// is part of that commit; a correct threshold under a misleading name is the
/// half-fix that reads as done.
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
