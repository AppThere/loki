// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Microbenchmark of the document open → first-paint pipeline.
//!
//! Times each stage on the acid DOCX fixture (a representative multi-page
//! document) so the open-latency hotspots are measurable headlessly:
//!
//! ```text
//! cargo run -p loki-acid --release --example load_bench
//! ```
//!
//! Stages: system-font scan (`FontResources::new`), import, Loro bridge
//! (`document_to_loro`), and the first paginated layout — with a cold vs. warm
//! font context so the per-open scan cost is isolated.

use std::io::Cursor;
use std::time::Instant;

use loki_doc_model::io::DocumentImport;
use loki_doc_model::loro_bridge::document_to_loro;
use loki_layout::{FontResources, LayoutMode, LayoutOptions, layout_document};
use loki_ooxml::{DocxImport, DocxImportOptions};

fn ms(label: &str, t: Instant) -> Instant {
    eprintln!(
        "  {label:<34} {:>8.1} ms",
        t.elapsed().as_secs_f64() * 1000.0
    );
    Instant::now()
}

fn main() {
    let bytes = loki_acid::Fixture::Docx.bytes();
    eprintln!("load_bench — acid_docx.docx ({} KiB)", bytes.len() / 1024);

    // Stage 1: system-font scan (the per-open cost paid by every new
    // DocumentState / throwaway FontResources).
    let t = Instant::now();
    let mut fonts = FontResources::new();
    let t = ms("FontResources::new (cold scan)", t);

    // A second scan, to show it is *not* cached across instances.
    let _fonts2 = FontResources::new();
    let _ = ms("FontResources::new (second instance)", t);

    // Stage 2: import.
    let t = Instant::now();
    let doc =
        DocxImport::import(Cursor::new(bytes), DocxImportOptions::default()).expect("import docx");
    let t = ms("DocxImport::import", t);

    // Stage 3: Loro bridge (editor seeds the CRDT from the imported document).
    let _loro = document_to_loro(&doc).expect("document_to_loro");
    ms("document_to_loro", t);

    // Stage 4: first paginated layout (reusing the warm font context — the path
    // a well-optimised editor should take).
    let opts = LayoutOptions {
        preserve_for_editing: true,
    };
    let t = Instant::now();
    let layout = layout_document(&mut fonts, &doc, LayoutMode::Paginated, 1.0, &opts);
    let t = ms("layout_document (warm fonts)", t);

    // Stage 4b: the same layout with a *fresh* font context — the cost the
    // editor pays today because each surface builds its own FontResources.
    let mut cold_fonts = FontResources::new();
    let _ = layout_document(&mut cold_fonts, &doc, LayoutMode::Paginated, 1.0, &opts);
    let _ = ms("layout_document (cold fonts incl. scan)", t);

    if let loki_layout::DocumentLayout::Paginated(p) = layout {
        eprintln!("  => {} page(s)", p.pages.len());
    }
}
