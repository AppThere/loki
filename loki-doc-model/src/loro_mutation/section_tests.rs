// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `insert_section_after`: the new section continues the source's page
//! setup, starts with one empty paragraph, lands at the right index, and an
//! out-of-range source is a typed refusal.

use loro::LoroDoc;

use super::{insert_section_after, section_of_block};
use crate::content::block::Block;
use crate::content::inline::Inline;
use crate::document::Document;
use crate::layout::page::PageOrientation;
use crate::loro_bridge::{document_to_loro, loro_to_document};
use crate::style::catalog::StyleId;

fn two_page_doc() -> LoroDoc {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(vec![Inline::Str("body".into())])];
    doc.sections[0].layout.orientation = PageOrientation::Landscape;
    doc.sections[0].layout.margins.left = crate::loki_primitives::units::Points::new(100.0);
    doc.sections[0].page_style = Some(StyleId::new("Wide"));
    document_to_loro(&doc).expect("to loro")
}

#[test]
fn new_section_continues_the_page_setup_with_one_empty_paragraph() {
    let loro = two_page_doc();
    let new_index = insert_section_after(&loro, 0).expect("insert");
    assert_eq!(new_index, 1);

    let doc = loro_to_document(&loro).expect("derive");
    assert_eq!(doc.sections.len(), 2);
    let new = &doc.sections[1];
    // Geometry and the named-style reference are the source's.
    assert_eq!(new.layout.orientation, PageOrientation::Landscape);
    assert!((new.layout.margins.left.value() - 100.0).abs() < 0.01);
    assert_eq!(new.page_style.as_ref().map(|s| s.as_str()), Some("Wide"));
    // One empty paragraph, so the section is editable.
    assert_eq!(new.blocks.len(), 1);
    assert!(matches!(&new.blocks[0], Block::Para(inl) if inl.is_empty()));
    // The source section is untouched.
    assert_eq!(doc.sections[0].blocks.len(), 1);
}

#[test]
fn out_of_range_source_is_a_typed_refusal() {
    let loro = two_page_doc();
    assert!(insert_section_after(&loro, 5).is_err());
    let doc = loro_to_document(&loro).expect("derive");
    assert_eq!(doc.sections.len(), 1, "a refusal must not write");
}

#[test]
fn section_of_block_walks_the_global_index() {
    let loro = two_page_doc();
    insert_section_after(&loro, 0).expect("insert");
    // Section 0 has one block ("body"); the new section 1 has one empty para.
    assert_eq!(section_of_block(&loro, 0), Some(0));
    assert_eq!(section_of_block(&loro, 1), Some(1));
    assert_eq!(section_of_block(&loro, 2), None, "past the last block");
}
