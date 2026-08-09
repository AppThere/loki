// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The residency **instrument**: a process-wide resident-texture-byte counter
//! (Spec 08 T2.5).
//!
//! # Why a counter and not a derivation
//!
//! Phase 0's texture figures were all derived from code with no profiler
//! (§3.6), and Spec 08 r1's three false capability claims came from exactly
//! that mode of working. A counter fed by the real allocation and release sites
//! is checkable: the budget becomes something a test asserts rather than
//! something a reader is asked to believe, and an on-device run can read the
//! same number without a second instrument being written for it.
//!
//! # Process-wide, and why that is the right scope
//!
//! GPU memory is a process resource, not a per-document one. Two open tabs
//! each holding a mounted window compete for the same budget, so a per-document
//! counter would report each of them as comfortable while the process was not.
//! The cost is that concurrent measurement is not meaningful — see
//! [`TextureResidency::reset`].
//!
//! # Balance is the property that matters
//!
//! Every `record_alloc` must be matched by exactly one `record_free`. An
//! unbalanced pair does not merely mis-report: [`TextureResidency::resident`]
//! drifts upward for the rest of the session, so a budget check made later
//! fails for a reason that has nothing to do with the pages actually mounted.
//! The Phase 2 bench asserts the counter returns to zero after each subject for
//! this reason, which is also its L08-022 order-dependence control — a leak
//! from one subject is exactly what would make the next one's figure wrong in a
//! plausible rather than an obvious way.

use std::sync::atomic::{AtomicU64, Ordering};

/// Bytes currently held by live page textures.
static RESIDENT: AtomicU64 = AtomicU64::new(0);
/// High-water mark of [`RESIDENT`] since the last reset.
static PEAK: AtomicU64 = AtomicU64::new(0);
/// Cumulative count of texture allocations recorded.
static ALLOCS: AtomicU64 = AtomicU64::new(0);
/// Cumulative count of texture releases recorded.
static FREES: AtomicU64 = AtomicU64::new(0);

/// A consistent-enough read of all four counters, for reporting.
///
/// Consistent-*enough*: the fields are read one at a time, so a snapshot taken
/// while tiles are being mounted may catch `resident` and `peak` a few bytes
/// apart. Reporting is the use; a budget decision reads
/// [`TextureResidency::resident`] directly.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct TextureResidencySnapshot {
    /// Bytes currently held by live page textures.
    pub resident: u64,
    /// High-water mark since the last reset.
    pub peak: u64,
    /// Allocations recorded since the last reset.
    pub allocs: u64,
    /// Releases recorded since the last reset.
    pub frees: u64,
}

impl TextureResidencySnapshot {
    /// `true` when every recorded allocation has been released.
    ///
    /// The check the Phase 2 bench runs between subjects: a `false` here means
    /// a later measurement is billed for an earlier one's tiles.
    #[must_use]
    pub fn is_balanced(self) -> bool {
        self.resident == 0 && self.allocs == self.frees
    }
}

/// Process-wide resident page-texture byte counter.
///
/// A namespace over static counters rather than an instance: the resource being
/// counted is process-wide (see the module docs), and threading a handle from
/// the wgpu paint callback out to a test would be a second mechanism for the
/// same fact.
pub struct TextureResidency;

impl TextureResidency {
    /// Records a newly allocated page texture of `bytes`.
    ///
    /// Call at the allocation site, not at the point the texture is first
    /// painted: the memory is committed by the allocation.
    pub fn record_alloc(bytes: u64) {
        let now = RESIDENT.fetch_add(bytes, Ordering::Relaxed) + bytes;
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        PEAK.fetch_max(now, Ordering::Relaxed);
    }

    /// Records the release of a page texture of `bytes`.
    ///
    /// Saturates at zero rather than wrapping. A saturating floor turns an
    /// unbalanced release into an under-report, which the `allocs == frees`
    /// half of [`TextureResidencySnapshot::is_balanced`] still catches; a
    /// wrapping one would read as ~16 exabytes resident and be dismissed as
    /// obviously broken instrumentation rather than investigated.
    pub fn record_free(bytes: u64) {
        RESIDENT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |cur| {
                Some(cur.saturating_sub(bytes))
            })
            .ok();
        FREES.fetch_add(1, Ordering::Relaxed);
    }

    /// Bytes currently held by live page textures.
    #[must_use]
    pub fn resident() -> u64 {
        RESIDENT.load(Ordering::Relaxed)
    }

    /// High-water mark of [`Self::resident`] since the last [`Self::reset`].
    #[must_use]
    pub fn peak() -> u64 {
        PEAK.load(Ordering::Relaxed)
    }

    /// All four counters at once.
    #[must_use]
    pub fn snapshot() -> TextureResidencySnapshot {
        TextureResidencySnapshot {
            resident: RESIDENT.load(Ordering::Relaxed),
            peak: PEAK.load(Ordering::Relaxed),
            allocs: ALLOCS.load(Ordering::Relaxed),
            frees: FREES.load(Ordering::Relaxed),
        }
    }

    /// Zeroes every counter.
    ///
    /// For measurement harnesses starting a fresh subject. Because the counters
    /// are process-wide, a reset from one thread clears another thread's
    /// in-flight accounting too — so measurement is sequential by construction,
    /// the same constraint `loki_bench::measure` carries for dhat.
    pub fn reset() {
        RESIDENT.store(0, Ordering::Relaxed);
        PEAK.store(0, Ordering::Relaxed);
        ALLOCS.store(0, Ordering::Relaxed);
        FREES.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
#[path = "counter_tests.rs"]
mod tests;
