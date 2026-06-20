// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Scaling benchmark for warm relayout — the per-keystroke layout cost.
//!
//! Every edit currently re-runs `layout_document` over the whole document.
//! The paragraph shaping cache makes unchanged paragraphs cheap to *shape*, but
//! the flow/pagination pass still rebuilds the entire positioned-item list and
//! the editing index every time. This benchmark grows a real document by
//! repeating its blocks and measures warm relayout at each size, so the
//! per-paragraph cost (and whether it warrants incremental layout) is concrete:
//!
//! ```text
//! cargo run -p loki-acid --release --example relayout_bench
//! ```

use std::io::Cursor;
use std::time::{Duration, Instant};

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentImport;
use loki_layout::{
    FontResources, LayoutMode, LayoutOptions, layout_document, layout_paginated_full,
    relayout_paginated_incremental,
};
use loki_ooxml::{DocxImport, DocxImportOptions};

const ITERS: usize = 7;

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Builds a **single-section** document whose blocks are every section's blocks
/// flattened and repeated `factor` times. Single-section is the incremental
/// path's eligibility (the editor's common case); the acid fixture itself has
/// several sections.
fn grow(base: &Document, factor: usize) -> Document {
    let all_blocks: Vec<_> = base
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter().cloned())
        .collect();
    let mut doc = base.clone();
    let mut first = doc.sections.first().cloned().unwrap_or_default();
    first.blocks = (0..factor)
        .flat_map(|_| all_blocks.iter().cloned())
        .collect();
    doc.sections = vec![first];
    doc
}

fn count_paragraphs(doc: &Document) -> usize {
    doc.sections.iter().map(|s| s.blocks.len()).sum()
}

/// Returns a copy of `doc` with one character flipped in block `idx` — a
/// length-preserving (height-preserving) single-block content edit, the common
/// keystroke case the incremental path optimises.
fn same_height_edit(doc: &Document, idx: usize) -> Document {
    let mut d = doc.clone();
    if let Some(section) = d.sections.first_mut()
        && let Some(Block::StyledPara(p)) = section.blocks.get_mut(idx)
    {
        for inline in p.inlines.iter_mut() {
            if let Inline::Str(text) = inline
                && !text.is_empty()
            {
                let mut chars: Vec<char> = text.chars().collect();
                chars[0] = if chars[0] == 'Z' { 'Y' } else { 'Z' };
                *text = chars.into_iter().collect();
                return d;
            }
        }
    }
    d
}

/// Median wall-clock of `ITERS` runs of `f`.
fn bench_median(mut f: impl FnMut()) -> Duration {
    let mut samples = Vec::with_capacity(ITERS);
    for _ in 0..ITERS {
        let t = Instant::now();
        f();
        samples.push(t.elapsed());
    }
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn main() {
    let bytes = loki_acid::Fixture::Docx.bytes();
    let base = DocxImport::import(Cursor::new(&bytes), DocxImportOptions::default())
        .expect("import acid docx");
    let opts = LayoutOptions {
        preserve_for_editing: true,
    };

    eprintln!(
        "relayout_bench — full vs incremental relayout, height-preserving \
         single-block edit ({ITERS} iters/size)\n"
    );
    eprintln!(
        "  {:>6}  {:>7}  {:>6}  {:>10}  {:>10}  {:>8}",
        "factor", "blocks", "pages", "full", "incremental", "speedup"
    );

    for factor in [1usize, 2, 4, 8, 16] {
        let doc = grow(&base, factor);
        let blocks = count_paragraphs(&doc);

        // Warm FontResources reused across iterations — models the editor, whose
        // shared context is warm after the first layout.
        let mut fonts = FontResources::new();
        let (prev_layout, reuse) = layout_paginated_full(&mut fonts, &doc, 1.0, &opts);
        let pages = prev_layout.pages.len();

        // The edit the editor would relayout after: one character in the middle.
        let edited = same_height_edit(&doc, blocks / 2);

        // Full relayout of the edited document (today's per-keystroke cost).
        let full = bench_median(|| {
            let _ = layout_document(&mut fonts, &edited, LayoutMode::Paginated, 1.0, &opts);
        });

        // Incremental relayout against the previous layout + reuse metadata.
        let mut fired = false;
        let incr = bench_median(|| {
            if relayout_paginated_incremental(
                &mut fonts,
                &edited,
                &doc,
                &prev_layout,
                &reuse,
                1.0,
                &opts,
            )
            .is_some()
            {
                fired = true;
            }
        });

        let speedup = ms(full) / ms(incr).max(0.0001);
        eprintln!(
            "  {factor:>6}  {blocks:>7}  {pages:>6}  {:>7.2} ms  {:>7.2} ms  {:>6.1}x{}",
            ms(full),
            ms(incr),
            speedup,
            if fired { "" } else { "  (fell back!)" },
        );
    }

    eprintln!("\n  Incremental reuses unchanged pages and re-flows only the edited region.");
    eprintln!("  Stage 1 clones reused pages; Arc-shared pages (Stage 2) remove that O(n) tail.");
}
