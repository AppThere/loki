// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Portable allocation-metric proof (Spec 06 M1): installs dhat's global
//! allocator and shows `loki_bench::measure` captures real, hardware-independent
//! allocation bytes/counts headless — the signal that backs continuous memory
//! tracking (decision D1). The per-tier baselines that diff these numbers land
//! in M3.
//!
//! `harness = false`: this is a plain binary, not a Criterion timed bench, so it
//! reports the portable memory signal rather than a distribution.
//!
//! Run: `cargo bench -p loki-bench --bench portable_alloc`

loki_bench::dhat_global_allocator!();

fn main() {
    // A trivial, deterministic portable workload: allocate a known collection.
    let stats = loki_bench::measure(|| {
        let v: Vec<u64> = (0..10_000).collect();
        std::hint::black_box(&v);
    });

    eprintln!(
        "portable_alloc — dhat allocation signal (headless, no GPU):\n  \
         total_bytes={} total_blocks={} max_bytes={} max_blocks={}",
        stats.total_bytes, stats.total_blocks, stats.max_bytes, stats.max_blocks,
    );

    // With the global allocator installed the workload must register a signal;
    // this is the M1 acceptance that the portable memory path works headless.
    assert!(
        stats.total_bytes > 0 && stats.total_blocks > 0,
        "dhat recorded no allocations — is the global allocator installed?",
    );
}
