// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Portable-axis smoke bench (Spec 06 M1): proves the Criterion harness runs
//! headless in the agent environment with no GPU. This is a placeholder target
//! — the per-operation portable benches (typing/layout/IO/export/style) land in
//! M2 and replace it.
//!
//! Run: `cargo bench -p loki-bench --bench portable_smoke`

use criterion::{Criterion, criterion_group, criterion_main};
use loki_bench::{Axis, Metric};
use std::hint::black_box;

fn portable_smoke(c: &mut Criterion) {
    // The portable axis is, by construction, agent-runnable — assert it so this
    // target documents the axis it belongs to.
    assert!(Metric::WallTime.axis() == Axis::DeviceBound);
    assert!(Axis::Portable.is_agent_runnable());

    c.bench_function("portable/smoke_sum", |b| {
        b.iter(|| {
            let sum: u64 = (0..black_box(1_000u64)).sum();
            black_box(sum)
        });
    });
}

criterion_group!(benches, portable_smoke);
criterion_main!(benches);
