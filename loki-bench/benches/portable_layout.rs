// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Portable layout bench (Spec 06 M2 / §6): allocations to lay out a document
//! (Parley text layout + pagination), across the scale tiers.
//!
//! `FontResources` is built once (the ~20 MB font scan is the editor's shared,
//! amortised cost — not charged per layout), and the paragraph cache is cleared
//! before each tier so the number is the **cold-layout** allocation cost, the
//! signal for "did layout get heavier."
//!
//! Run: `cargo bench -p loki-bench --bench portable_layout`

loki_bench::dhat_global_allocator!();

#[path = "support/mod.rs"]
mod support;

use loki_bench::{AllocStats, measure};
use loki_layout::{FontResources, LayoutMode, LayoutOptions, layout_document};
use std::hint::black_box;

fn main() {
    support::header("layout — allocations to lay out a document (Paginated, cold paragraph cache)");

    let mut resources = FontResources::new();
    let options = LayoutOptions {
        preserve_for_editing: true,
        spell: None,
    };

    let mut worst = AllocStats::default();
    for &(name, paras) in support::DOC_TIERS {
        let doc = support::build_doc(paras, support::WORDS_PER_PARA);
        resources.clear_paragraph_cache();
        let stats = measure(|| {
            let layout =
                layout_document(&mut resources, &doc, LayoutMode::Paginated, 1.0, &options);
            black_box(&layout);
        });
        support::report_row(&format!("{name} ({paras} paras)"), stats);
        worst = stats;
    }

    assert!(worst.total_bytes > 0, "layout recorded no allocations");
}
