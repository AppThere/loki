// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Edit-path benchmark: the full per-keystroke recompute the editor performs.
//!
//! Mirrors the two dominant steps of
//! `loki_text::editing::state::apply_mutation_and_relayout`:
//!
//! 1. `loro_to_document` — reconstruct the whole `Document` from the CRDT.
//! 2. `layout_document`  — re-lay-out the whole document (Paginated).
//!
//! A single character is inserted and then removed each iteration so the
//! document length stays constant across the run while still paying for a real
//! mutation. Because both steps walk the entire document, this benchmark is the
//! headline measurement for the "O(n) per keystroke" finding: compare the
//! per-element throughput across the sweep — flat cost-per-paragraph confirms
//! the whole-document recompute.
//!
//! Run: `cargo bench -p loki-layout --bench edit_path`

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use loki_doc_model::loro_bridge::{IncrementalReader, document_to_loro, loro_to_document};
use loki_doc_model::loro_mutation::{delete_text, insert_text};
use loki_layout::{FontResources, LayoutMode, LayoutOptions, layout_document};
use std::hint::black_box;

#[path = "support/mod.rs"]
mod support;

fn bench_keystroke(c: &mut Criterion) {
    // Shared font resources, as the editor keeps one per session.
    let mut resources = FontResources::new();
    let options = LayoutOptions {
        preserve_for_editing: true,
        spell: None,
        ..Default::default()
    };

    let mut group = c.benchmark_group("edit_path_keystroke");
    for &n in support::SWEEP {
        let doc = support::build_doc(n, support::WORDS_PER_PARA);
        let loro = match document_to_loro(&doc) {
            Ok(d) => d,
            Err(e) => panic!("document_to_loro failed for n={n}: {e}"),
        };

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(
            BenchmarkId::new("loro_to_document+layout", n),
            &n,
            |b, _| {
                b.iter(|| {
                    // One keystroke into the first paragraph, then undo it so the
                    // document length is stable across iterations.
                    let _ = insert_text(&loro, 0, 0, "x");
                    let derived = match loro_to_document(&loro) {
                        Ok(d) => d,
                        Err(_) => return,
                    };
                    let layout = layout_document(
                        &mut resources,
                        &derived,
                        LayoutMode::Paginated,
                        1.0,
                        &options,
                    );
                    black_box(&layout);
                    let _ = delete_text(&loro, 0, 0, 1);
                });
            },
        );
    }
    group.finish();
}

/// Same keystroke pipeline, but reconstructing the document with
/// [`IncrementalReader`] (re-derive only the changed block) instead of a full
/// `loro_to_document`. Compare against `bench_keystroke` to see the win from
/// incremental reconstruction on top of the paragraph shaping cache.
fn bench_keystroke_incremental(c: &mut Criterion) {
    let mut resources = FontResources::new();
    let options = LayoutOptions {
        preserve_for_editing: true,
        spell: None,
        ..Default::default()
    };

    let mut group = c.benchmark_group("edit_path_incremental");
    for &n in support::SWEEP {
        let doc = support::build_doc(n, support::WORDS_PER_PARA);
        let loro = match document_to_loro(&doc) {
            Ok(d) => d,
            Err(e) => panic!("document_to_loro failed for n={n}: {e}"),
        };
        let mut reader = match IncrementalReader::seed(&loro) {
            Ok(r) => r,
            Err(e) => panic!("IncrementalReader::seed failed for n={n}: {e}"),
        };

        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("incremental+layout", n), &n, |b, _| {
            b.iter(|| {
                let _ = insert_text(&loro, 0, 0, "x");
                if let Ok(derived) = reader.update(&loro) {
                    let layout = layout_document(
                        &mut resources,
                        derived,
                        LayoutMode::Paginated,
                        1.0,
                        &options,
                    );
                    black_box(&layout);
                }
                let _ = delete_text(&loro, 0, 0, 1);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_keystroke, bench_keystroke_incremental);
criterion_main!(benches);
