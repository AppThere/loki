// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Render an ACID fixture through Loki's own pipeline to a PDF, for visual
//! comparison against a canonical (Word / LibreOffice) render.
//!
//! Usage:
//!   cargo run -p loki-acid --example render_acid_pdf -- <in.docx|in.odt> <out.pdf>
//!
//! Reuses `loki-layout` for pagination (the same engine the editor/GPU renderer
//! use), so the PDF geometry matches Loki's on-screen layout. GPU-free.

use std::io::Cursor;

use loki_doc_model::Document;
use loki_odf::OdtImport;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: render_acid_pdf <in.docx|in.odt> <out.pdf>");
        std::process::exit(2);
    }
    let bytes = std::fs::read(&args[1]).expect("read input");
    let doc: Document = if args[1].to_ascii_lowercase().ends_with(".odt") {
        use loki_doc_model::io::DocumentImport;
        OdtImport::import(Cursor::new(bytes), Default::default()).expect("import odt")
    } else {
        DocxImporter::new(DocxImportOptions::default())
            .run(Cursor::new(bytes))
            .expect("import docx")
            .document
    };

    let mut out = Vec::new();
    loki_pdf::export_document(&doc, &loki_pdf::PdfXOptions::default(), &mut out)
        .expect("export pdf");
    std::fs::write(&args[2], &out).expect("write pdf");
    eprintln!("wrote {} ({} bytes)", args[2], out.len());
}
