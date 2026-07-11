// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the table-of-contents CRDT mutations.

use super::*;
use crate::content::attr::NodeAttr;
use crate::content::block::Block;
use crate::content::inline::Inline;
use crate::document::Document;
use crate::layout::page::PageLayout;
use crate::layout::section::Section;
use crate::loro_bridge::{document_to_loro, loro_to_document};

fn heading(level: u8, text: &str) -> Block {
    Block::Heading(level, NodeAttr::default(), vec![Inline::Str(text.into())])
}

/// A doc with three headings and some body paragraphs.
fn sample_loro() -> LoroDoc {
    let mut doc = Document::new();
    doc.sections = vec![Section::with_layout_and_blocks(
        PageLayout::default(),
        vec![
            heading(1, "Introduction"),
            Block::Para(vec![Inline::Str("Opening paragraph.".into())]),
            heading(2, "Details"),
            heading(1, "Conclusion"),
        ],
    )];
    document_to_loro(&doc).expect("to loro")
}

fn toc_entry_texts(block: &Block) -> Vec<String> {
    let Block::TableOfContents(toc) = block else {
        panic!("expected a TableOfContents block");
    };
    toc.body
        .iter()
        .map(|b| match b {
            Block::StyledPara(p) => crate::content::toc::inline_plain_text(&p.inlines),
            _ => String::new(),
        })
        .collect()
}

#[test]
fn insert_builds_a_toc_from_headings_after_the_target_block() {
    let loro = sample_loro();
    // Insert after block 0 (the first heading).
    let new_idx = insert_table_of_contents(&loro, 0, Some("Contents"), 3).expect("insert");
    assert_eq!(new_idx, 1);

    let doc = loro_to_document(&loro).expect("rebuild");
    let toc = &doc.sections[0].blocks[1];
    let texts = toc_entry_texts(toc);
    // Title + the three headings (level 2 is within depth 3).
    assert_eq!(
        texts,
        vec!["Contents", "Introduction", "Details", "Conclusion"]
    );
    // The original blocks are all still present (TOC was inserted, not replaced).
    assert_eq!(doc.sections[0].blocks.len(), 5);
}

#[test]
fn insert_respects_the_depth_bound() {
    let loro = sample_loro();
    insert_table_of_contents(&loro, 0, None, 1).expect("insert");
    let doc = loro_to_document(&loro).expect("rebuild");
    let texts = toc_entry_texts(&doc.sections[0].blocks[1]);
    // Depth 1 → only the two level-1 headings, no title.
    assert_eq!(texts, vec!["Introduction", "Conclusion"]);
}

#[test]
fn refresh_rebuilds_the_snapshot_after_headings_change() {
    let loro = sample_loro();
    let idx = insert_table_of_contents(&loro, 0, Some("Contents"), 3).expect("insert");

    // Add a new heading to the document, then refresh the TOC in place.
    let doc = loro_to_document(&loro).expect("rebuild");
    let toc_index = first_toc_block_index(&doc.sections).expect("a toc exists");
    assert_eq!(toc_index, idx);

    // Append a heading via a fresh block after the TOC.
    insert_block_after(&loro, idx, &heading(1, "Appendix")).expect("append heading");
    refresh_table_of_contents(&loro, toc_index, Some("Contents"), 3).expect("refresh");

    let doc = loro_to_document(&loro).expect("rebuild");
    let texts = toc_entry_texts(&doc.sections[0].blocks[toc_index]);
    assert!(
        texts.contains(&"Appendix".to_string()),
        "refreshed TOC must include the new heading: {texts:?}"
    );
}

#[test]
fn refresh_is_a_no_op_on_a_non_toc_block() {
    let loro = sample_loro();
    // Block 1 is a plain paragraph, not a TOC — refresh must not touch it.
    refresh_table_of_contents(&loro, 1, None, 3).expect("no-op");
    let doc = loro_to_document(&loro).expect("rebuild");
    match &doc.sections[0].blocks[1] {
        Block::Para(inlines) => {
            assert_eq!(
                crate::content::toc::inline_plain_text(inlines),
                "Opening paragraph."
            );
        }
        other => panic!("block 1 must be unchanged, got {other:?}"),
    }
}

#[test]
fn first_toc_index_is_none_without_a_toc() {
    let doc = loro_to_document(&sample_loro()).expect("rebuild");
    assert_eq!(first_toc_block_index(&doc.sections), None);
}
