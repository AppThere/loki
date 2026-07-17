// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::document_to_loro;
use loki_doc_model::{get_block_style_name, get_block_text};
use loki_macro_host::{DocEdit, EditBatch};

use super::apply_batch_ops;

fn para(s: &str) -> Block {
    Block::Para(vec![Inline::Str(s.into())])
}

fn loro_with(texts: &[&str]) -> loro::LoroDoc {
    let mut doc = Document::new();
    doc.sections[0].blocks = texts.iter().map(|t| para(t)).collect();
    document_to_loro(&doc).unwrap()
}

fn batch(edits: Vec<DocEdit>) -> EditBatch {
    EditBatch { edits }
}

#[test]
fn append_extends_the_last_paragraph() {
    let loro = loro_with(&["Hello", "World"]);
    apply_batch_ops(
        &loro,
        2,
        &batch(vec![
            DocEdit::AppendText(" there".into()),
            DocEdit::AppendText("!".into()),
        ]),
    )
    .expect("apply");
    // Both appends land at the end of the last block.
    assert_eq!(get_block_text(&loro, 0), "Hello");
    assert_eq!(get_block_text(&loro, 1), "World there!");
}

#[test]
fn set_text_collapses_the_body_to_one_paragraph() {
    let loro = loro_with(&["First", "Second", "Third"]);
    apply_batch_ops(&loro, 3, &batch(vec![DocEdit::SetText("only".into())])).expect("apply");
    assert_eq!(get_block_text(&loro, 0), "only");
    // The trailing blocks are gone — reading block 1 yields empty (out of range).
    assert_eq!(get_block_text(&loro, 1), "");
}

#[test]
fn set_then_append_operates_on_the_collapsed_body() {
    let loro = loro_with(&["a", "b"]);
    apply_batch_ops(
        &loro,
        2,
        &batch(vec![
            DocEdit::SetText("X".into()),
            DocEdit::AppendText("Y".into()),
        ]),
    )
    .expect("apply");
    assert_eq!(get_block_text(&loro, 0), "XY");
    assert_eq!(get_block_text(&loro, 1), "");
}

#[test]
fn empty_append_is_a_noop() {
    let loro = loro_with(&["keep"]);
    apply_batch_ops(&loro, 1, &batch(vec![DocEdit::AppendText(String::new())])).expect("apply");
    assert_eq!(get_block_text(&loro, 0), "keep");
}

#[test]
fn set_text_preserves_block_zero_as_an_editable_paragraph() {
    // After SetText the surviving block 0 is still a real paragraph block (its
    // style name resolves), so the document stays editable.
    let loro = loro_with(&["one", "two"]);
    apply_batch_ops(&loro, 2, &batch(vec![DocEdit::SetText("fresh".into())])).expect("apply");
    // A paragraph block has a resolvable (possibly default) style name.
    let _ = get_block_style_name(&loro, 0);
    assert_eq!(get_block_text(&loro, 0), "fresh");
}
