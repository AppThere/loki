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
/// immediate, nameable failure: **repositions must not advance on an event where
/// nothing moved.**
pub(super) static REPOSITIONS: AtomicU64 = AtomicU64::new(0);

/// Comparisons the driver has performed since the last reset.
///
/// # Why this replaces a threshold rather than re-deriving one
///
/// `REPOSITION_BURST_WARN = 30` was "half a second at 60 Hz" — a heuristic, and
/// the only thing a frame driver allows: every frame runs a comparison whether
/// or not anything moved, so a settled popover emits `Ignore` continuously and
/// only a *rate* distinguishes jitter from work.
///
/// The event driver makes an exact statement available instead. Each event runs
/// **one** comparison, which yields **at most one** reposition, so
/// `repositions() <= events()` is an arithmetic invariant rather than a taste.
/// Exceeding it is impossible without the comparison being re-entered inside a
/// single event — which is the reactive loop D-15 names, and the only loop an
/// event driver can have.
///
/// So the threshold is gone rather than rescaled, and the rename obligation is
/// discharged by deletion: there is no "burst" left to misname. That is a better
/// outcome than a new number, and it is available only because the driver's
/// shape changed — the same reason this program moved from clocks to counters.
pub(super) static EVENTS: AtomicU64 = AtomicU64::new(0);

/// Records that the driver is about to run one comparison.
///
/// Called by [`super::super::anchor_scope::PopoverAnchor::reposition`] before it
/// compares, so the invariant above has a denominator. A consumer that never
/// repositions never calls this, and `0 <= 0` holds trivially.
pub fn note_event() {
    EVENTS.fetch_add(1, Ordering::Relaxed);
}

/// Comparisons performed since the last reset.
#[must_use]
pub fn events() -> u64 {
    EVENTS.load(Ordering::Relaxed)
}

/// Repositions since the last reset. Assert this is unchanged across frames in
/// which nothing moved.
#[must_use]
pub fn repositions() -> u64 {
    REPOSITIONS.load(Ordering::Relaxed)
}

/// Resets both counters.
///
/// Both, because the invariant is a relation between them: resetting one alone
/// would leave `repositions() <= events()` comparing figures from different
/// windows, which is the two-sources-one-input shape (L08-029).
pub fn reset_repositions() {
    REPOSITIONS.store(0, Ordering::Relaxed);
    EVENTS.store(0, Ordering::Relaxed);
}
