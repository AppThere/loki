// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Portable heap-allocation measurement (Spec 06 §5/§7, decision D1).

/// Portable heap-allocation metrics for a measured region, captured with dhat.
///
/// Bytes and block counts barely move across machines, so they are the primary
/// *continuously tracked* memory signal — diffable across machines and over time,
/// unlike wall-clock or RSS (decision D1). Serializable derives land with the
/// committed baseline in M3; M1 keeps the type dependency-light.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct AllocStats {
    /// Total bytes allocated over the region (cumulative, not net).
    pub total_bytes: u64,
    /// Total number of allocations over the region (cumulative).
    pub total_blocks: u64,
    /// Peak live bytes at any instant during the region.
    pub max_bytes: u64,
    /// Peak live allocations at any instant during the region.
    pub max_blocks: u64,
}

/// Runs `workload` under a dhat heap-profiling session and returns the portable
/// [`AllocStats`] it produced.
///
/// **Requires the dhat global allocator** to be installed in the running binary
/// via [`crate::dhat_global_allocator!`]. Without it dhat records nothing and the
/// returned stats are all zero — the harness still runs, it just has no signal,
/// so bench binaries must opt in.
///
/// The dhat profiler is process-global and testing-mode allows only one at a
/// time, so `measure` builds and drops its own session per call; sequential calls
/// are therefore safe, concurrent ones are not.
pub fn measure<F: FnOnce()>(workload: F) -> AllocStats {
    let _profiler = dhat::Profiler::builder().testing().build();
    workload();
    let stats = dhat::HeapStats::get();
    AllocStats {
        total_bytes: stats.total_bytes,
        total_blocks: stats.total_blocks,
        // dhat reports the live-peak fields as `usize`; widen to the stable u64
        // the baseline (M3) will serialize. usize -> u64 never truncates.
        max_bytes: stats.max_bytes as u64,
        max_blocks: stats.max_blocks as u64,
    }
}

#[cfg(test)]
#[path = "memory_tests.rs"]
mod tests;
