// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! PDF / PDF-X export for the Loki document suite.
//!
//! [`export_document`] lays a [`loki_doc_model::Document`] out with the shared
//! `loki-layout` engine (the same engine that drives on-screen rendering) and
//! serialises the resulting positioned pages to a print-ready PDF/X file using
//! the `pdf-writer` crate.
//!
//! All three print-oriented conformance levels — PDF/X-1a, PDF/X-3 and
//! PDF/X-4 — are supported via [`PdfXLevel`]. Text and graphics are emitted in
//! DeviceCMYK with an attached [`OutputIntent`]; fonts referenced by the
//! layout are embedded as `CIDFontType2` programs, satisfying the PDF/X
//! requirement that all fonts be embedded.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod build;
pub mod color;
pub mod error;
pub mod fonts;
pub mod metadata;
pub mod options;
pub mod page;

use loki_doc_model::Document;
use loki_layout::{
    DocumentLayout, FontResources, LayoutMode, LayoutOptions, PaginatedLayout, layout_document,
};

pub use error::PdfError;
pub use options::{OutputIntent, PdfXLevel, PdfXOptions};

/// Lays out `doc` and writes it to `out` as a PDF/X file.
///
/// The document is flowed in [`LayoutMode::Paginated`] so the PDF reproduces
/// the document's own page geometry. Returns [`PdfError::NoPages`] if the
/// document produces no pages.
pub fn export_document(
    doc: &Document,
    options: &PdfXOptions,
    out: &mut Vec<u8>,
) -> Result<(), PdfError> {
    let mut resources = FontResources::new();
    let layout = layout_document(
        &mut resources,
        doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions::default(),
    );

    let paginated = match layout {
        DocumentLayout::Paginated(p) => p,
        // Paginated mode always returns a paginated layout, but guard anyway.
        _ => return Err(PdfError::NoPages),
    };

    *out = build_pdf(&paginated, doc, options)?;
    Ok(())
}

/// Serialises an already-computed paginated layout to PDF/X bytes.
///
/// Exposed separately so a caller that has already laid the document out (e.g.
/// the editor) can reuse its layout instead of recomputing one.
pub fn build_pdf(
    layout: &PaginatedLayout,
    doc: &Document,
    options: &PdfXOptions,
) -> Result<Vec<u8>, PdfError> {
    if layout.pages.is_empty() {
        return Err(PdfError::NoPages);
    }
    build::write_document(layout, doc, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use loki_doc_model::content::block::Block;
    use loki_doc_model::content::inline::Inline;

    fn sample_doc() -> Document {
        let mut doc = Document::new_blank();
        doc.meta.title = Some("PDF Test".into());
        if let Some(sec) = doc.first_section_mut() {
            sec.blocks.clear();
            sec.blocks.push(Block::Para(vec![Inline::Str(
                "Hello PDF/X from Loki.".into(),
            )]));
        }
        doc
    }

    #[test]
    fn exports_a_valid_pdf_header() {
        let mut out = Vec::new();
        export_document(&sample_doc(), &PdfXOptions::default(), &mut out).expect("export");
        assert!(out.starts_with(b"%PDF-1."), "missing PDF header");
        assert!(out.windows(5).any(|w| w == b"%%EOF"), "missing EOF");
    }

    #[test]
    fn declares_pdfx_marker() {
        let mut out = Vec::new();
        let opts = PdfXOptions {
            level: PdfXLevel::X4,
            ..Default::default()
        };
        export_document(&sample_doc(), &opts, &mut out).expect("export");
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("PDF/X-4") || out.windows(7).any(|w| w == b"PDF/X-4"));
    }

    #[test]
    fn embeds_fonts_and_output_intent() {
        let mut out = Vec::new();
        export_document(&sample_doc(), &PdfXOptions::default(), &mut out).expect("export");
        let bytes = &out;
        let has = |needle: &[u8]| bytes.windows(needle.len()).any(|w| w == needle);
        // The document has text, so a font program must be embedded.
        assert!(has(b"FontFile2"), "font program not embedded");
        assert!(has(b"CIDFontType2"), "descendant font not written");
        // PDF/X output intent and conformance marker must be present.
        assert!(has(b"OutputIntent"), "output intent missing");
        assert!(has(b"GTS_PDFX"), "PDF/X marker missing");
        // X-1a (default) requires a trailer ID.
        assert!(has(b"/ID"), "trailer /ID missing");
    }
}
