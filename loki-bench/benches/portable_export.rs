// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Portable export bench (Spec 06 M2 / §6): document-emission allocations across
//! formats, DOCX vs ODT, over the scale tiers — so format-emission cost is
//! comparable and a regression in either is visible.
//!
//! PDF/EPUB emission additionally drives the layout pipeline (`loki-layout`);
//! those are portable too and land as a follow-up within the export target so
//! this bench stays a focused format-comparison of the two office writers.
//!
//! Run: `cargo bench -p loki-bench --bench portable_export`

loki_bench::dhat_global_allocator!();

#[path = "support/mod.rs"]
mod support;

use loki_bench::{AllocStats, measure};
use loki_doc_model::io::DocumentExport;
use loki_odf::OdtExport;
use loki_ooxml::DocxExport;
use std::hint::black_box;
use std::io::Cursor;

fn main() {
    support::header("export — DOCX vs ODT emission allocations");

    let mut worst = AllocStats::default();
    for &(name, paras) in support::DOC_TIERS {
        let doc = support::build_doc(paras, support::WORDS_PER_PARA);

        let docx = measure(|| {
            let mut buf = Cursor::new(Vec::new());
            // DocxExport's Options is the unit type; `()` avoids clippy::unit_arg.
            DocxExport::export(&doc, &mut buf, ()).expect("docx export");
            black_box(buf.into_inner().len());
        });
        support::report_row(&format!("{name} docx"), docx);

        let odt = measure(|| {
            let mut buf = Cursor::new(Vec::new());
            OdtExport::export(&doc, &mut buf, Default::default()).expect("odt export");
            black_box(buf.into_inner().len());
        });
        support::report_row(&format!("{name} odt"), odt);
        worst = odt;
    }

    assert!(worst.total_bytes > 0, "export recorded no allocations");
}
