// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for CRDT history compaction (`loro_bridge::compact`, memory-audit
//! Finding 6): compaction must shrink the oplog without changing the
//! document, and the compacted doc must remain fully editable.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{
    compact_history, compact_in_place, document_to_loro, loro_to_document,
};
use loki_doc_model::{delete_text, insert_text};

/// A document with one paragraph, put through `n` insert+delete keystroke
/// pairs so the oplog grows while the content stays fixed.
fn edited_doc(n: usize) -> loro::LoroDoc {
    let mut doc = Document::new();
    doc.sections[0]
        .blocks
        .push(Block::Para(vec![Inline::Str("stable text".into())]));
    let loro = document_to_loro(&doc).expect("document_to_loro");
    for _ in 0..n {
        insert_text(&loro, 0, 0, "x").expect("insert");
        delete_text(&loro, 0, 0, 1).expect("delete");
    }
    loro
}

#[test]
fn compact_history_truncates_oplog_and_preserves_content() {
    let loro = edited_doc(500);
    let before = loro_to_document(&loro).expect("read before");
    let ops_before = loro.len_ops();

    let compacted = compact_history(&loro).expect("compact_history");

    let after = loro_to_document(&compacted).expect("read after");
    assert_eq!(
        before.sections[0].blocks, after.sections[0].blocks,
        "compaction must not change the document content"
    );
    assert!(
        compacted.len_ops() < ops_before / 10,
        "oplog must shrink by at least 10x (was {ops_before}, now {})",
        compacted.len_ops()
    );
}

#[test]
fn compacted_doc_remains_editable_with_marks() {
    let loro = edited_doc(100);
    let compacted = compact_history(&loro).expect("compact_history");

    // Edits must apply to the compacted doc and round-trip; the text-style
    // config (mark expand behaviour) must have been re-registered.
    insert_text(&compacted, 0, 0, "hello ").expect("insert after compaction");
    let doc = loro_to_document(&compacted).expect("read");
    match &doc.sections[0].blocks[0] {
        Block::Para(inlines) => {
            let text: String = inlines
                .iter()
                .map(|i| match i {
                    Inline::Str(s) => s.clone(),
                    other => panic!("unexpected inline: {other:?}"),
                })
                .collect();
            assert_eq!(text, "hello stable text");
        }
        other => panic!("expected Para, got {other:?}"),
    }
}

#[test]
fn compact_history_is_repeatable() {
    // Compact, edit, compact again — the second pass must also succeed and
    // keep the state (guards against one-shot assumptions in the swap).
    let loro = edited_doc(100);
    let once = compact_history(&loro).expect("first compaction");
    insert_text(&once, 0, 0, "more ").expect("edit");
    let twice = compact_history(&once).expect("second compaction");
    let doc = loro_to_document(&twice).expect("read");
    match &doc.sections[0].blocks[0] {
        Block::Para(inlines) => {
            assert!(matches!(&inlines[0], Inline::Str(s) if s.starts_with("more ")));
        }
        other => panic!("expected Para, got {other:?}"),
    }
}

#[test]
fn compact_in_place_preserves_history_and_content() {
    let loro = edited_doc(200);
    let before = loro_to_document(&loro).expect("read before");
    let ops_before = loro.len_ops();

    compact_in_place(&loro);

    // History is preserved (this is the memory-representation compaction),
    // the document is unchanged, and the doc stays editable.
    assert_eq!(loro.len_ops(), ops_before, "history must be preserved");
    let after = loro_to_document(&loro).expect("read after");
    assert_eq!(before.sections[0].blocks, after.sections[0].blocks);
    insert_text(&loro, 0, 0, "y").expect("still editable");
}
