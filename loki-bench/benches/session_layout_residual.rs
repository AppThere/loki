// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Inactive-tab layout retention guard (Spec 06 BM-8 / memory-audit F3,
//! deferred-features plan 6.1).
//!
//! A stashed tab session used to retain its `Arc<PaginatedLayout>` — Parley
//! layouts + byte maps for every paragraph, megabytes per inactive tab,
//! scaling with tab count. The 6.1 fix stashes only the model (`Document`)
//! and recomputes the layout on restore. This bench measures the residual
//! live heap of both stash shapes and asserts the model-only shape stays far
//! below the layout-retaining one — the tripwire for the layout ever
//! creeping back into `DocSession`.
//!
//! Run: `cargo bench -p loki-bench --bench session_layout_residual`

loki_bench::dhat_global_allocator!();

#[path = "support/mod.rs"]
mod support;

use loki_bench::residual_after;
use loki_doc_model::Document;
use loki_layout::{FontResources, LayoutOptions, PaginatedLayout, layout_paginated_full};
use std::hint::black_box;
use std::sync::Arc;

/// Simulated inactive-tab count.
const TABS: usize = 6;
/// Paragraphs per stashed document (a working document).
const PARAS: usize = 120;

fn main() {
    let doc = support::build_doc(PARAS, support::WORDS_PER_PARA);
    let mut resources = FontResources::new();
    let options = LayoutOptions {
        preserve_for_editing: true,
        spell: None,
        ..Default::default()
    };
    // Warm the font stack outside both measured regions.
    let _ = layout_paginated_full(&mut resources, &doc, 1.0, &options);

    // Pre-6.1 shape: each stashed session pins its layout.
    let mut retained: Vec<(Arc<Document>, Option<Arc<PaginatedLayout>>)> = Vec::new();
    resources.clear_paragraph_cache();
    let with_layout = residual_after(TABS, || {
        let (layout, _) = layout_paginated_full(&mut resources, &doc, 1.0, &options);
        retained.push((Arc::new(doc.clone()), Some(Arc::new(layout))));
    });
    black_box(&retained);
    drop(retained);

    // 6.1 shape: the session keeps only the model; the layout drops on stash.
    let mut model_only: Vec<(Arc<Document>, Option<Arc<PaginatedLayout>>)> = Vec::new();
    resources.clear_paragraph_cache();
    let without_layout = residual_after(TABS, || {
        let (layout, _) = layout_paginated_full(&mut resources, &doc, 1.0, &options);
        black_box(&layout);
        model_only.push((Arc::new(doc.clone()), None));
    });
    black_box(&model_only);

    let per_tab_saving = with_layout
        .curr_bytes
        .saturating_sub(without_layout.curr_bytes)
        / TABS as u64;
    eprintln!(
        "session_layout_residual — {TABS} stashed tabs × {PARAS}-para document:\n  \
         layout retained:  {} B live\n  \
         model-only stash: {} B live\n  \
         ≈ {per_tab_saving} B saved per inactive tab",
        with_layout.curr_bytes, without_layout.curr_bytes,
    );

    // The retained shape must dwarf the model-only shape; if this ever fails,
    // either the layout crept back into the stash or PaginatedLayout shrank
    // to irrelevance (update the guard if it is genuinely the latter).
    assert!(
        with_layout.curr_bytes > without_layout.curr_bytes * 2,
        "retaining layouts no longer dominates the stash residual: {} vs {} B",
        with_layout.curr_bytes,
        without_layout.curr_bytes,
    );
}
