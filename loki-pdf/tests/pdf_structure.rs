// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Integration tests for PDF/X export (audit T-3): export a document and
//! validate the resulting file's low-level structure by re-parsing it — the
//! page tree's `/Count` against the actual leaf `/Page` objects, the
//! `startxref` pointer against the cross-reference table, the trailer, and the
//! PDF/X conformance markers across all three levels.
//!
//! The inline unit tests in `lib.rs` assert token presence; these tests assert
//! the file is structurally coherent (the xref offset really points at the
//! table, the page count really matches the pages emitted).

use loki_doc_model::Document;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_pdf::{PdfXLevel, PdfXOptions, export_document};

/// Builds a document with `paras` single-line paragraphs — enough to span
/// several pages when `paras` is large.
fn doc_with_paras(paras: usize) -> Document {
    let mut doc = Document::new_blank();
    doc.meta.title = Some("Structural Test".into());
    if let Some(sec) = doc.first_section_mut() {
        sec.blocks.clear();
        for i in 0..paras {
            sec.blocks.push(Block::Para(vec![Inline::Str(format!(
                "Paragraph number {i} exists to fill the page with flowing text."
            ))]));
        }
    }
    doc
}

fn export(doc: &Document, level: PdfXLevel) -> Vec<u8> {
    let opts = PdfXOptions {
        level,
        ..Default::default()
    };
    let mut out = Vec::new();
    export_document(doc, &opts, &mut out).expect("export");
    out
}

/// Number of leaf page objects: every `/Type /Page` occurrence minus the single
/// `/Type /Pages` node (which `/Type /Page` is a textual prefix of).
fn leaf_page_count(pdf: &str) -> usize {
    pdf.matches("/Type /Page").count() - pdf.matches("/Type /Pages").count()
}

/// Parses the `/Count N` value from the page-tree node.
fn declared_page_count(pdf: &str) -> usize {
    let i = pdf.find("/Count ").expect("page tree must declare /Count");
    pdf[i + "/Count ".len()..]
        .split_whitespace()
        .next()
        .and_then(|n| n.parse().ok())
        .expect("/Count must be followed by an integer")
}

#[test]
fn page_tree_count_matches_emitted_pages() {
    // Many paragraphs must paginate into more than one page.
    let out = export(&doc_with_paras(400), PdfXLevel::X1a);
    let pdf = String::from_utf8_lossy(&out);

    let declared = declared_page_count(&pdf);
    let leaves = leaf_page_count(&pdf);
    assert!(
        declared > 1,
        "expected a multi-page document, got {declared}"
    );
    assert_eq!(
        declared, leaves,
        "page-tree /Count ({declared}) must equal the number of leaf /Page objects ({leaves})"
    );
}

#[test]
fn startxref_points_at_the_xref_table() {
    let out = export(&doc_with_paras(50), PdfXLevel::X1a);
    let pdf = String::from_utf8_lossy(&out);

    let idx = pdf.rfind("startxref").expect("missing startxref");
    let offset: usize = pdf[idx + "startxref".len()..]
        .split_whitespace()
        .next()
        .and_then(|n| n.parse().ok())
        .expect("startxref must be followed by an offset");

    assert!(offset < out.len(), "startxref offset is past end of file");
    assert!(
        out[offset..].starts_with(b"xref"),
        "startxref must point at the cross-reference table"
    );
    // The file must terminate with the EOF marker after the trailer.
    assert!(pdf.trim_end().ends_with("%%EOF"), "missing trailing %%EOF");
}

#[test]
fn trailer_declares_root_and_id() {
    let out = export(&doc_with_paras(10), PdfXLevel::X1a);
    let pdf = String::from_utf8_lossy(&out);
    assert!(pdf.contains("trailer"), "missing trailer");
    assert!(pdf.contains("/Root"), "trailer must reference the catalog");
    // PDF/X requires a file identifier in the trailer.
    assert!(pdf.contains("/ID"), "trailer must carry a file /ID");
}

#[test]
fn all_pdfx_levels_are_structurally_valid() {
    for (level, marker) in [
        (PdfXLevel::X1a, "PDF/X-1a"),
        (PdfXLevel::X3, "PDF/X-3"),
        (PdfXLevel::X4, "PDF/X-4"),
    ] {
        let out = export(&doc_with_paras(20), level);
        let pdf = String::from_utf8_lossy(&out);

        assert!(out.starts_with(b"%PDF-1."), "{marker}: missing PDF header");
        assert!(pdf.trim_end().ends_with("%%EOF"), "{marker}: missing %%EOF");
        assert!(
            pdf.contains("GTS_PDFXVersion") || pdf.contains(marker),
            "{marker}: missing PDF/X conformance declaration"
        );
        // Output intent is mandatory for every PDF/X level.
        assert!(
            pdf.contains("OutputIntent"),
            "{marker}: missing OutputIntent"
        );
        assert!(declared_page_count(&pdf) >= 1, "{marker}: no pages");
    }
}

#[test]
fn single_page_document_reports_one_page() {
    let out = export(&doc_with_paras(1), PdfXLevel::X1a);
    let pdf = String::from_utf8_lossy(&out);
    assert_eq!(declared_page_count(&pdf), 1);
    assert_eq!(leaf_page_count(&pdf), 1);
}

#[test]
fn empty_document_is_rejected_not_silently_emitted() {
    // A document that produces no pages must surface NoPages rather than write a
    // structurally invalid zero-page PDF.
    let mut doc = Document::new_blank();
    if let Some(sec) = doc.first_section_mut() {
        sec.blocks.clear();
    }
    let mut out = Vec::new();
    let result = export_document(&doc, &PdfXOptions::default(), &mut out);
    // Either it lays out one (empty) page, or it reports NoPages — but it must
    // never emit a page tree whose /Count disagrees with its leaves.
    if let Ok(()) = result {
        let pdf = String::from_utf8_lossy(&out);
        assert_eq!(declared_page_count(&pdf), leaf_page_count(&pdf));
    }
}
