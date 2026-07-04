// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Leak detection (Spec 06 M4 / §7): residual live-heap measurement plus a pure
//! verdict on whether that residual *scales with repetitions* (a leak) or stays
//! flat (bounded).
//!
//! Where [`measure`](crate::measure) reports the cumulative allocation of a
//! region, leak detection watches the **live heap still held after** a
//! open→edit→close cycle. Run the cycle once and again N times: if the residual
//! is flat, nothing leaked; if it grows ~linearly with N, a document was
//! retained (an `Arc` cycle) or a cache never evicted — the §7 culprits.
//!
//! The residual measurement needs dhat's global allocator (installed in the bench
//! binary via [`dhat_global_allocator!`](crate::dhat_global_allocator)); the
//! [`classify_leak`] verdict is pure and unit-tested.

/// Live heap still allocated when a measured region returns — the leak signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ResidualStats {
    /// Bytes allocated during the region that are still live at its end.
    pub curr_bytes: u64,
    /// Allocations during the region still live at its end.
    pub curr_blocks: u64,
}

/// Runs `cycle` `reps` times under a dhat session and returns the heap still live
/// afterward. A leak-free cycle leaves a flat residual as `reps` grows; a leak
/// grows it ~linearly. Returns a zeroed residual if the dhat allocator is not
/// installed (the bench binary must opt in).
pub fn residual_after<F: FnMut()>(reps: usize, mut cycle: F) -> ResidualStats {
    let _profiler = dhat::Profiler::builder().testing().build();
    for _ in 0..reps {
        cycle();
    }
    let stats = dhat::HeapStats::get();
    ResidualStats {
        curr_bytes: stats.curr_bytes as u64,
        curr_blocks: stats.curr_blocks as u64,
    }
}

/// Whether residual heap scaled with repetitions (a leak) or stayed bounded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeakVerdict {
    /// Residual stayed within the bounded envelope — no per-cycle leak.
    Bounded,
    /// Residual grew past the envelope — retained work accumulates.
    Leaking,
}

impl LeakVerdict {
    /// `true` for [`LeakVerdict::Leaking`].
    #[must_use]
    pub fn leaks(self) -> bool {
        self == LeakVerdict::Leaking
    }
}

/// Classifies a leak from residual live bytes at a low and a high repetition
/// count.
///
/// `baseline` is the residual after few cycles (one-time init + one cycle's
/// worth); `scaled` is the residual after many. Bounded work keeps `scaled`
/// within `slack_bytes` of `baseline` (the one-time cost is paid once either
/// way); a leak makes `scaled` climb with the repetition count, well past
/// `slack_bytes`.
#[must_use]
pub fn classify_leak(baseline: u64, scaled: u64, slack_bytes: u64) -> LeakVerdict {
    if scaled > baseline.saturating_add(slack_bytes) {
        LeakVerdict::Leaking
    } else {
        LeakVerdict::Bounded
    }
}

#[cfg(test)]
#[path = "leak_tests.rs"]
mod tests;
