// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The §12 import-only sources through the real matrix: Markdown and
//! Fountain bytes in, DOCX/PDF out, with the template catalogs merged so the
//! output carries the styles the importers reference.

use std::io::Cursor;

use loki_convert::{ConvertOptions, Format, convert};
use loki_doc_model::io::DocumentImport;
use loki_ooxml::DocxImport;

const MARKDOWN: &[u8] = b"# Report\n\nSome *body* text.\n\n- one\n- two\n";

const FOUNTAIN: &[u8] = b"Title: TEST\n\nINT. ROOM - DAY\n\nALEX\nHello.\n";

#[test]
fn markdown_converts_to_docx_with_styles_and_lists() {
    let out = convert(
        Format::Markdown,
        MARKDOWN,
        Format::Docx,
        &ConvertOptions::default(),
    )
    .expect("md->docx should convert");

    // The output must parse as DOCX and carry the structure through.
    let doc = DocxImport::import(Cursor::new(out.bytes), Default::default())
        .expect("output parses as DOCX");
    let blocks = &doc.sections[0].blocks;
    assert!(!blocks.is_empty());
    // The list items still know they are a list (the §10 writers at work).
    let list_items = blocks
        .iter()
        .filter(|b| match b {
            loki_doc_model::content::block::Block::StyledPara(p) => p
                .direct_para_props
                .as_ref()
                .is_some_and(|pp| pp.list_id.is_some()),
            _ => false,
        })
        .count();
    assert_eq!(list_items, 2, "the bullet items degraded on the way out");
}

#[test]
fn fountain_converts_to_pdf() {
    let out = convert(
        Format::Fountain,
        FOUNTAIN,
        Format::Pdf,
        &ConvertOptions::default(),
    )
    .expect("fountain->pdf should convert");
    assert!(out.bytes.starts_with(b"%PDF"), "output is not a PDF");
}

#[test]
fn markdown_is_not_a_target() {
    let err = convert(
        Format::Docx,
        MARKDOWN,
        Format::Markdown,
        &ConvertOptions::default(),
    )
    .expect_err("docx->md must be a typed refusal");
    assert!(matches!(
        err,
        loki_convert::ConvertError::ConversionUnsupported { .. }
    ));
}
