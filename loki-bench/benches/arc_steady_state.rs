// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Arc steady-state guard (Spec 06 M3 / §7 / audit §4).
//!
//! The tiered render-cache win holds resident memory down by sharing
//! `vello::Renderer`, `PaginatedLayout`, and `FontResources` behind `Arc` — one
//! careless value-clone reverts that toward per-instance duplication. `Arc::clone`
//! only bumps a refcount, so sharing a resource allocates **nothing on the heap**;
//! a deep clone would allocate the whole resource. This guard measures the share
//! path on a real shared resource (`FontResources`, constructed headless) and
//! asserts it stays at **zero allocations** — the deterministic tripwire for a
//! share→clone regression.
//!
//! `vello::Renderer` needs a GPU, so its identical guard is device-bound (M5);
//! the mechanism proven here is the same.
//!
//! Run: `cargo bench -p loki-bench --bench arc_steady_state`

loki_bench::dhat_global_allocator!();

use loki_bench::measure;
use loki_layout::FontResources;
use std::hint::black_box;
use std::sync::Arc;

fn main() {
    // Build the shared resource once (its font scan is outside the measured region).
    let shared: Arc<FontResources> = Arc::new(FontResources::new());

    // Share it 10_000 times via Arc::clone — a refcount bump, never a heap alloc.
    let stats = measure(|| {
        for _ in 0..10_000 {
            let handle = Arc::clone(&shared);
            black_box(&handle);
        }
    });

    eprintln!(
        "arc_steady_state — sharing FontResources via Arc::clone (10_000×):\n  \
         bytes={} allocs={}  (both must be 0 — shared steady state holds)",
        stats.total_bytes, stats.total_blocks,
    );

    assert_eq!(
        stats.total_blocks, 0,
        "Arc::clone allocated — the shared steady state regressed toward \
         per-instance duplication (audit §4)",
    );
    assert_eq!(
        stats.total_bytes, 0,
        "Arc::clone must not allocate heap bytes"
    );
}
