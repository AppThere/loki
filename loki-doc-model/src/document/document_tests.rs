// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::*;
use crate::content::block::Block;
use crate::layout::page::{PageLayout, PageSize};

fn hr() -> Block {
    Block::HorizontalRule
}

fn make_doc_with_sections(blocks_per_section: &[usize]) -> Document {
    let mut doc = Document::new();
    doc.sections.clear();
    for &count in blocks_per_section {
        let blocks = (0..count).map(|_| hr()).collect();
        doc.sections.push(Section::with_layout_and_blocks(
            PageLayout::default(),
            blocks,
        ));
    }
    doc
}

#[test]
fn document_new_has_one_section() {
    let doc = Document::new();
    assert_eq!(doc.sections.len(), 1);
    assert!(doc.meta.title.is_none());
    assert!(doc.source.is_none());
}

#[test]
fn document_two_sections_different_sizes() {
    let mut doc = Document::new();

    let mut layout2 = PageLayout::default();
    layout2.page_size = PageSize::a4();
    let section2 = Section::with_layout_and_blocks(layout2, vec![]);
    doc.sections.push(section2);

    assert_eq!(doc.sections.len(), 2);
    assert_ne!(
        doc.sections[0].layout.page_size,
        doc.sections[1].layout.page_size,
    );
}

#[test]
fn section_count_empty_document() {
    let doc = make_doc_with_sections(&[]);
    assert_eq!(doc.section_count(), 0);
    assert!(doc.section_at(0).is_none());
}

#[test]
fn section_at_single_section() {
    let doc = make_doc_with_sections(&[2]);
    assert_eq!(doc.sections().len(), 1);
    assert!(doc.section_at(0).is_some());
    assert!(doc.section_at(1).is_none());
}

#[test]
fn sections_mut_allows_modification() {
    let mut doc = make_doc_with_sections(&[1]);
    doc.sections_mut()[0].blocks.push(hr());
    assert_eq!(doc.section_at(0).unwrap().blocks.len(), 2);
}

#[test]
fn block_count_flat_empty_document() {
    let doc = make_doc_with_sections(&[]);
    assert_eq!(doc.block_count_flat(), 0);
    assert!(doc.block_at_flat(0).is_none());
}

#[test]
fn block_at_flat_single_section_three_blocks() {
    let doc = make_doc_with_sections(&[3]);
    assert_eq!(doc.block_count_flat(), 3);
    assert!(doc.block_at_flat(2).is_some());
    assert!(doc.block_at_flat(3).is_none());
}

#[test]
fn block_at_flat_two_sections_two_blocks_each() {
    let doc = make_doc_with_sections(&[2, 2]);
    assert_eq!(doc.block_count_flat(), 4);
    assert!(doc.block_at_flat(2).is_some());
    assert!(doc.block_at_flat(4).is_none());
}

#[test]
fn flat_index_to_section_block_first_block() {
    let doc = make_doc_with_sections(&[2, 2]);
    assert_eq!(doc.flat_index_to_section_block(0), Some((0, 0)));
}

#[test]
fn flat_index_to_section_block_crosses_section_boundary() {
    let doc = make_doc_with_sections(&[2, 2]);
    assert_eq!(doc.flat_index_to_section_block(2), Some((1, 0)));
}

#[test]
fn flat_index_to_section_block_out_of_range() {
    let doc = make_doc_with_sections(&[2, 2]);
    assert!(doc.flat_index_to_section_block(4).is_none());
}

#[test]
fn blocks_flat_yields_all_blocks_in_order() {
    let doc = make_doc_with_sections(&[2, 3]);
    let count = doc.blocks_flat().count();
    assert_eq!(count, 5);
}
