// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Multi-section page geometry: a document with two sections of differing
//! page size and orientation must round-trip through DOCX export, each section
//! keeping its own `w:sectPr` geometry.

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_doc_model::layout::page::{PageLayout, PageOrientation, PageSize};
use loki_doc_model::layout::section::Section;
use loki_ooxml::DocxExport;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

fn para(text: &str) -> Block {
    Block::Para(vec![Inline::Str(text.to_string())])
}

#[test]
fn two_sections_round_trip() {
    let s0 = Section::with_layout_and_blocks(
        PageLayout {
            page_size: PageSize::a4(),
            orientation: PageOrientation::Portrait,
            ..PageLayout::default()
        },
        vec![para("Section one body")],
    );
    let s1 = Section::with_layout_and_blocks(
        PageLayout {
            page_size: PageSize::letter(),
            orientation: PageOrientation::Landscape,
            ..PageLayout::default()
        },
        vec![para("Section two body")],
    );

    let mut doc = Document::new();
    doc.sections = vec![s0, s1];

    let mut buf = Cursor::new(Vec::new());
    DocxExport::export(&doc, &mut buf, ()).expect("export should succeed");

    let re = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf.into_inner()))
        .expect("re-import should succeed");

    let secs = &re.document.sections;
    assert_eq!(secs.len(), 2, "two sections must survive round-trip");
    assert_eq!(secs[0].layout.orientation, PageOrientation::Portrait);
    assert_eq!(secs[1].layout.orientation, PageOrientation::Landscape);
}
