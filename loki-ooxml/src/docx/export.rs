// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX export — public entry point.
//!
//! Calls into `write::assembly` to assemble a conformant `.docx` ZIP.
//! See ADR-0007 for design decisions (Tier 3 fidelity, `Package::write`).

use std::io::{Seek, Write};

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;

use crate::docx::write::assembly::{DocxKind, assemble_docx, assemble_docx_kind};
use crate::error::OoxmlError;

/// Unit struct that implements [`DocumentExport`] for DOCX.
pub struct DocxExport;

impl DocumentExport for DocxExport {
    type Error = OoxmlError;
    type Options = ();

    fn export(
        doc: &Document,
        writer: impl Write + Seek,
        _options: Self::Options,
    ) -> Result<(), Self::Error> {
        assemble_docx(doc, writer)
    }
}

/// Unit struct that implements [`DocumentExport`] for a Word **template**
/// (`.dotx`). Identical to [`DocxExport`] except the main part carries the
/// template content type, so Office opens it as a template (creating a new
/// document based on it) rather than editing the file in place.
pub struct DocxTemplateExport;

impl DocumentExport for DocxTemplateExport {
    type Error = OoxmlError;
    type Options = ();

    fn export(
        doc: &Document,
        writer: impl Write + Seek,
        _options: Self::Options,
    ) -> Result<(), Self::Error> {
        assemble_docx_kind(doc, writer, DocxKind::Template)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn export_empty_document_produces_zip() {
        let doc = Document::new();
        let mut buf = Cursor::new(Vec::<u8>::new());
        DocxExport::export(&doc, &mut buf, ()).expect("export failed");
        // A ZIP starts with the local file header signature PK\x03\x04.
        let bytes = buf.into_inner();
        assert!(bytes.len() > 4, "output is too short to be a ZIP");
        assert_eq!(&bytes[..2], b"PK", "output does not begin with ZIP magic");
    }

    #[test]
    fn export_document_with_heading_and_para() {
        use loki_doc_model::content::attr::NodeAttr;
        use loki_doc_model::content::block::Block;
        use loki_doc_model::content::inline::Inline;

        let mut doc = Document::new();
        let section = doc.sections.first_mut().unwrap();
        section.blocks.push(Block::Heading(
            1,
            NodeAttr::default(),
            vec![Inline::Str("Hello world".into())],
        ));
        section
            .blocks
            .push(Block::Para(vec![Inline::Str("Some text.".into())]));

        let mut buf = Cursor::new(Vec::<u8>::new());
        DocxExport::export(&doc, &mut buf, ()).expect("export failed");
        let bytes = buf.into_inner();
        assert_eq!(&bytes[..2], b"PK");
    }

    #[test]
    fn export_bullet_list_produces_zip() {
        use loki_doc_model::content::block::Block;
        use loki_doc_model::content::inline::Inline;

        let mut doc = Document::new();
        let section = doc.sections.first_mut().unwrap();
        section.blocks.push(Block::BulletList(vec![
            vec![Block::Para(vec![Inline::Str("Item one".into())])],
            vec![Block::Para(vec![Inline::Str("Item two".into())])],
        ]));

        let mut buf = Cursor::new(Vec::<u8>::new());
        DocxExport::export(&doc, &mut buf, ()).expect("export failed");
        let bytes = buf.into_inner();
        assert_eq!(&bytes[..2], b"PK");
    }

    #[test]
    fn export_ordered_list_produces_zip() {
        use loki_doc_model::content::block::{Block, ListAttributes};
        use loki_doc_model::content::inline::Inline;

        let mut doc = Document::new();
        let section = doc.sections.first_mut().unwrap();
        section.blocks.push(Block::OrderedList(
            ListAttributes::default(),
            vec![
                vec![Block::Para(vec![Inline::Str("First".into())])],
                vec![Block::Para(vec![Inline::Str("Second".into())])],
            ],
        ));

        let mut buf = Cursor::new(Vec::<u8>::new());
        DocxExport::export(&doc, &mut buf, ()).expect("export failed");
        let bytes = buf.into_inner();
        assert_eq!(&bytes[..2], b"PK");
    }
}
