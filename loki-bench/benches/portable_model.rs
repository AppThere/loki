// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Portable model bench (Spec 06 M2 / §6): allocations to reconstruct the whole
//! `Document` from the Loro CRDT — the model half of typing latency.
//!
//! The editor rebuilds the document from the CRDT on the edit path
//! (`apply_mutation_and_relayout` → `loro_to_document`), so this is the
//! per-keystroke model-rebuild allocation cost, swept across the scale tiers.
//!
//! Run: `cargo bench -p loki-bench --bench portable_model`

loki_bench::dhat_global_allocator!();

#[path = "support/mod.rs"]
mod support;

use loki_bench::{AllocStats, measure};
use loki_doc_model::{document_to_loro, loro_to_document};
use std::hint::black_box;

fn main() {
    support::header(
        "model — allocations to reconstruct the Document from the CRDT (per-keystroke rebuild)",
    );

    let mut worst = AllocStats::default();
    for &(name, paras) in support::DOC_TIERS {
        let doc = support::build_doc(paras, support::WORDS_PER_PARA);
        let loro = document_to_loro(&doc).expect("document_to_loro");
        let stats = measure(|| {
            let rebuilt = loro_to_document(&loro).expect("loro_to_document");
            black_box(&rebuilt);
        });
        support::report_row(&format!("{name} ({paras} paras)"), stats);
        worst = stats;
    }

    assert!(
        worst.total_bytes > 0,
        "model rebuild recorded no allocations"
    );
}
