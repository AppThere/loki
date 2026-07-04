// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The dhat allocation-measurement plumbing (Spec 06 §5/§7). Verified without
//! the global allocator installed, so it asserts the invariants that hold either
//! way; the nonzero-signal proof is the `portable_alloc` bench, which opts in.

use super::*;

#[test]
fn measure_runs_the_workload_and_returns_well_formed_stats() {
    let mut ran = false;
    let stats = measure(|| {
        let v: Vec<u64> = (0..256).collect();
        std::hint::black_box(&v);
        ran = true;
    });
    assert!(ran, "measure must run the workload exactly once");
    // dhat invariant: live peak never exceeds the cumulative total — true whether
    // or not the global allocator is installed (0 <= 0 when it is not).
    assert!(stats.max_bytes <= stats.total_bytes);
    assert!(stats.max_blocks <= stats.total_blocks);
}
