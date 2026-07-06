// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the double-Enter list-exit predicate (plan 4b.1): pressing Enter
//! on an empty, top-level list item exits the list instead of adding a bullet.

use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::document_to_loro;
use loki_doc_model::style::StyleId;
use loki_doc_model::style::list_style::ListId;
use loki_doc_model::style::props::ParaProps;
use loki_doc_model::{NodeAttr, PathStep};

use super::is_empty_list_item_exit;
use crate::editing::cursor::DocumentPosition;

/// A top-level list paragraph (`list_id = "L1"`, level 0) holding `text`
/// (empty `text` → a truly empty paragraph).
fn list_item(text: &str) -> Block {
    let inlines = if text.is_empty() {
        Vec::new()
    } else {
        vec![Inline::Str(text.into())]
    };
    Block::StyledPara(StyledParagraph {
        style_id: Some(StyleId::new("Normal")),
        direct_para_props: Some(Box::new(ParaProps {
            list_id: Some(ListId::new("L1")),
            list_level: Some(0),
            ..ParaProps::default()
        })),
        direct_char_props: None,
        inlines,
        attr: NodeAttr::default(),
    })
}

fn loro_with_blocks(blocks: Vec<Block>) -> loro::LoroDoc {
    let mut doc = Document::new();
    doc.sections[0].blocks = blocks;
    document_to_loro(&doc).unwrap()
}

/// A plain caret at the start of top-level block `block`.
fn caret(block: usize) -> DocumentPosition {
    DocumentPosition::top_level(0, block, 0)
}

#[test]
fn exits_an_empty_list_item() {
    let ldoc = loro_with_blocks(vec![list_item("")]);
    assert!(is_empty_list_item_exit(&ldoc, &caret(0), false));
}

#[test]
fn does_not_exit_a_nonempty_list_item() {
    let ldoc = loro_with_blocks(vec![list_item("x")]);
    assert!(
        !is_empty_list_item_exit(&ldoc, &caret(0), false),
        "a list item with text should split, not exit"
    );
}

#[test]
fn does_not_exit_a_plain_empty_paragraph() {
    let ldoc = loro_with_blocks(vec![Block::Para(Vec::new())]);
    assert!(
        !is_empty_list_item_exit(&ldoc, &caret(0), false),
        "an empty non-list paragraph splits normally"
    );
}

#[test]
fn a_selection_suppresses_the_list_exit() {
    // With a selection active, Enter replaces the selection (split), never exits.
    let ldoc = loro_with_blocks(vec![list_item("")]);
    assert!(!is_empty_list_item_exit(&ldoc, &caret(0), true));
}

#[test]
fn nested_list_item_is_not_exited() {
    // A caret with a non-empty path (inside a cell / note) is excluded — the
    // list block API is top-level only.
    let ldoc = loro_with_blocks(vec![list_item("")]);
    let mut nested = caret(0);
    nested.path = vec![PathStep::Cell { cell: 0, block: 0 }];
    assert!(!is_empty_list_item_exit(&ldoc, &nested, false));
}
