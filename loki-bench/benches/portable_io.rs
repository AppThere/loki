// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Portable open/save bench (Spec 06 M2 / §6): DOCX import (open) and export
//! (save) allocations, across the scale tiers.
//!
//! Measures the parse/serialize halves of open and save (the IO wall-clock tail
//! is device-bound and lives on the device axis). Save is measured first to
//! produce the bytes the open measurement reads; the input slice is borrowed
//! (not cloned) inside the open region so only the import allocates.
//!
//! Run: `cargo bench -p loki-bench --bench portable_io`

loki_bench::dhat_global_allocator!();

#[path = "support/mod.rs"]
mod support;

use loki_bench::{AllocStats, measure};
use loki_doc_model::io::{DocumentExport, DocumentImport};
use loki_ooxml::{DocxExport, DocxImport};
use std::hint::black_box;
use std::io::Cursor;

fn main() {
    support::header("open/save — DOCX export (save) + import (open) allocations");

    let mut worst = AllocStats::default();
    for &(name, paras) in support::DOC_TIERS {
        let doc = support::build_doc(paras, support::WORDS_PER_PARA);

        // Save: serialize the document to DOCX bytes.
        let mut bytes = Vec::new();
        let save = measure(|| {
            let mut buf = Cursor::new(Vec::new());
            // DocxExport's Options is the unit type; pass `()` (not
            // `Default::default()`, which trips clippy::unit_arg).
            DocxExport::export(&doc, &mut buf, ()).expect("docx export");
            bytes = buf.into_inner();
        });
        support::report_row(&format!("{name} save (docx)"), save);

        // Open: parse those bytes back (borrow the slice, so the clone isn't charged).
        let open = measure(|| {
            let imported = DocxImport::import(Cursor::new(bytes.as_slice()), Default::default())
                .expect("docx import");
            black_box(&imported);
        });
        support::report_row(&format!("{name} open (docx)"), open);
        worst = open;
    }

    assert!(worst.total_bytes > 0, "open/save recorded no allocations");
}
