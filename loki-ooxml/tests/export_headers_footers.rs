// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Round-trip coverage for DOCX header/footer export, focusing on the
//! even-page variant which only survives when the exporter emits
//! `word/settings.xml` with `<w:evenAndOddHeaders/>` (ECMA-376 §17.10.1).

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_doc_model::layout::header_footer::{HeaderFooter, HeaderFooterKind};
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::layout::section::Section;
use loki_ooxml::DocxExport;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

/// Builds a one-section document carrying a default + even header and footer,
/// each with a distinctive text string.
fn doc_with_even_hf() -> Document {
    let para = |s: &str| HeaderFooter {
        kind: HeaderFooterKind::Default,
        blocks: vec![Block::Para(vec![Inline::Str(s.to_string())])],
    };

    let layout = PageLayout {
        header: Some(para("Default Header")),
        footer: Some(para("Default Footer")),
        header_even: Some(HeaderFooter {
            kind: HeaderFooterKind::Even,
            blocks: vec![Block::Para(vec![Inline::Str("Even Header".to_string())])],
        }),
        footer_even: Some(HeaderFooter {
            kind: HeaderFooterKind::Even,
            blocks: vec![Block::Para(vec![Inline::Str("Even Footer".to_string())])],
        }),
        ..PageLayout::default()
    };

    let section = Section::with_layout_and_blocks(
        layout,
        vec![Block::Para(vec![Inline::Str("Body text".to_string())])],
    );

    let mut doc = Document::new();
    doc.sections = vec![section];
    doc
}

/// Collects every plain string in a header/footer's blocks.
fn hf_text(hf: Option<&HeaderFooter>) -> Vec<String> {
    let Some(hf) = hf else {
        return Vec::new();
    };
    hf.blocks
        .iter()
        .flat_map(|b| {
            let inlines = match b {
                Block::Para(i) => i.as_slice(),
                Block::StyledPara(sp) => sp.inlines.as_slice(),
                _ => &[],
            };
            inlines.iter().filter_map(|i| {
                if let Inline::Str(s) = i {
                    Some(s.clone())
                } else {
                    None
                }
            })
        })
        .collect()
}

#[test]
fn even_page_headers_footers_round_trip() {
    let doc = doc_with_even_hf();

    let mut buf = Cursor::new(Vec::new());
    DocxExport::export(&doc, &mut buf, ()).expect("export should succeed");
    let bytes = buf.into_inner();

    let re = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(bytes))
        .expect("re-import should succeed");

    let layout = &re.document.sections.last().unwrap().layout;

    assert!(
        hf_text(layout.header.as_ref()).contains(&"Default Header".to_string()),
        "default header must survive round-trip"
    );
    assert!(
        hf_text(layout.footer.as_ref()).contains(&"Default Footer".to_string()),
        "default footer must survive round-trip"
    );
    assert!(
        hf_text(layout.header_even.as_ref()).contains(&"Even Header".to_string()),
        "even-page header must survive round-trip (requires w:evenAndOddHeaders)"
    );
    assert!(
        hf_text(layout.footer_even.as_ref()).contains(&"Even Footer".to_string()),
        "even-page footer must survive round-trip (requires w:evenAndOddHeaders)"
    );

    // A document with no first-page variant must NOT spuriously gain one
    // (w:titlePg must be first-page-only, not emitted for even-only docs).
    assert!(
        layout.header_first.is_none(),
        "even-only document must not produce a first-page header"
    );
    assert!(
        layout.footer_first.is_none(),
        "even-only document must not produce a first-page footer"
    );
}
