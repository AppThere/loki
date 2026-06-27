// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Layout-scaling benchmark: how `layout_document` cost grows with document
//! length, in both Paginated and Reflow modes.
//!
//! This measures the layout half of the per-keystroke pipeline in isolation
//! (the Loro-traversal half is covered by `edit_path.rs`). The `FontResources`
//! is constructed once and reused across all iterations — matching the editor's
//! `shared_font_resources` production path — so the ~20 MB font scan is *not*
//! charged per layout here. A separate `font_resources_new` benchmark times
//! that scan on its own, because the renderer path currently pays it per
//! generation (see the performance assessment).
//!
//! Run: `cargo bench -p loki-layout --bench layout_scaling`

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use loki_layout::{FontResources, LayoutMode, LayoutOptions, layout_document};
use std::hint::black_box;

#[path = "support/mod.rs"]
mod support;

/// Reflow viewport width in points (~ a phone/narrow-window content column).
const REFLOW_WIDTH_PT: f32 = 360.0;

fn bench_layout(c: &mut Criterion) {
    // One shared FontResources for the whole sweep — the editor reuses a single
    // instance, so charging the font scan once here reflects real cost.
    let mut resources = FontResources::new();
    let options = LayoutOptions {
        preserve_for_editing: true,
        spell: None,
    };

    let mut group = c.benchmark_group("layout_document");
    for &n in support::SWEEP {
        let doc = support::build_doc(n, support::WORDS_PER_PARA);
        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(BenchmarkId::new("paginated", n), &doc, |b, doc| {
            b.iter(|| {
                let layout =
                    layout_document(&mut resources, doc, LayoutMode::Paginated, 1.0, &options);
                black_box(&layout);
            });
        });

        group.bench_with_input(BenchmarkId::new("reflow", n), &doc, |b, doc| {
            b.iter(|| {
                let layout = layout_document(
                    &mut resources,
                    doc,
                    LayoutMode::Reflow {
                        available_width: REFLOW_WIDTH_PT,
                    },
                    1.0,
                    &options,
                );
                black_box(&layout);
            });
        });
    }
    group.finish();
}

/// Times `FontResources::new()` on its own — the system-font scan the renderer
/// currently repeats per generation instead of sharing.
fn bench_font_resources_new(c: &mut Criterion) {
    c.bench_function("font_resources_new", |b| {
        b.iter(|| black_box(FontResources::new()));
    });
}

criterion_group!(benches, bench_layout, bench_font_resources_new);
criterion_main!(benches);
