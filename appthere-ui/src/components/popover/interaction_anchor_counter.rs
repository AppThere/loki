// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The reposition counter and its burst threshold (Spec 08 T4.1).
//!
//! Split from `interaction_anchor.rs` at the 300-line ceiling (r72), when
//! settling D-15 grew the driver's reasoning. A natural seam: that module is
//! about *what a movement means*, this is the instrument watching for the one
//! hazard the comparison itself can create.

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
/// **This derivation assumes a frame driver, and there is none** (see the module
/// docs). When the event-driven driver lands, the unit becomes repositions *per
/// event* and this number must be **re-derived rather than rescaled**: 30 is
/// half a second of frames, and there is no defensible way to read that as a
/// count of events. Left as-is because nothing drives the counter yet, and a
/// number changed ahead of its consumer is a number nobody can check.
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
