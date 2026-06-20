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

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentImport;
use loki_layout::{FontResources, LayoutMode, LayoutOptions, layout_document};
use loki_ooxml::{DocxImport, DocxImportOptions};

const ITERS: usize = 7;

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Builds a document with the first section's blocks repeated `factor` times.
fn grow(base: &Document, factor: usize) -> Document {
    let mut doc = base.clone();
    if let Some(first) = doc.sections.first_mut() {
        let original = first.blocks.clone();
        first.blocks.clear();
        for _ in 0..factor {
            first.blocks.extend(original.iter().cloned());
        }
    }
    doc
}

fn count_paragraphs(doc: &Document) -> usize {
    doc.sections.iter().map(|s| s.blocks.len()).sum()
}

fn main() {
    let bytes = loki_acid::Fixture::Docx.bytes();
    let base = DocxImport::import(Cursor::new(&bytes), DocxImportOptions::default())
        .expect("import acid docx");
    let opts = LayoutOptions {
        preserve_for_editing: true,
    };

    eprintln!("relayout_bench — warm relayout vs document size ({ITERS} iters/size)\n");
    eprintln!(
        "  {:>6}  {:>7}  {:>6}  {:>9}  {:>10}",
        "factor", "blocks", "pages", "median", "per-block"
    );

    for factor in [1usize, 2, 4, 8, 16] {
        let doc = grow(&base, factor);
        let blocks = count_paragraphs(&doc);

        // One warm FontResources reused across iterations — this models the
        // editor, where the shared context is warm after the first layout.
        let mut fonts = FontResources::new();
        let pages = match layout_document(&mut fonts, &doc, LayoutMode::Paginated, 1.0, &opts) {
            loki_layout::DocumentLayout::Paginated(p) => p.pages.len(),
            _ => 0,
        };

        let mut samples = Vec::with_capacity(ITERS);
        for _ in 0..ITERS {
            // Editing invalidates the paragraph that changed; model the common
            // case (caches warm) by keeping the cache populated between passes.
            let t = Instant::now();
            let _ = layout_document(&mut fonts, &doc, LayoutMode::Paginated, 1.0, &opts);
            samples.push(t.elapsed());
        }
        samples.sort_unstable();
        let median = samples[samples.len() / 2];
        eprintln!(
            "  {factor:>6}  {blocks:>7}  {pages:>6}  {:>7.2} ms  {:>7.1} µs",
            ms(median),
            ms(median) / blocks as f64 * 1000.0,
        );
    }

    eprintln!("\n  Warm relayout is O(blocks): every edit rebuilds the whole positioned-item");
    eprintln!("  list + editing index. 'per-block' is the incremental-layout target.");
}
