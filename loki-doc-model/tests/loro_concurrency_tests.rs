// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Concurrent-edit, merge-convergence, undo/redo, and pathological-state tests
//! for the `loki-doc-model` Loro CRDT bridge and mutation layer (audit T-6).
//!
//! The bridge round-trip is covered by `loro_bridge_tests.rs`; the single-peer
//! mutation semantics by `loro_mutation_tests.rs`. This file exercises the
//! property that actually justifies using a CRDT: two peers that diverge and
//! then exchange their updates **converge to byte-identical state**, regardless
//! of the order edits were applied.
//!
//! Convergence is asserted on `LoroDoc::get_deep_value()` — the fully resolved
//! document value. Two CRDT replicas that have seen the same set of operations
//! must produce equal deep values; this is a stronger check than comparing
//! reconstructed `Document`s (which `loro_to_document` could normalise).

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::loro_mutation::{
    delete_text, get_block_text, insert_text, merge_block, split_block,
};
use loro::{ExportMode, LoroDoc, UndoManager};

/// Builds a document whose section 0 holds `paras` paragraphs, the i-th
/// paragraph containing the text `"Block{i}"`.
fn doc_with_paras(paras: usize) -> Document {
    let mut doc = Document::new();
    if let Some(sec) = doc.first_section_mut() {
        for i in 0..paras {
            sec.blocks
                .push(Block::Para(vec![Inline::Str(format!("Block{i}"))]));
        }
    }
    doc
}

/// Exchanges all operations between two replicas so both observe the full
/// op set, then commits each. After this returns the two docs must converge.
fn sync(a: &LoroDoc, b: &LoroDoc) {
    a.commit();
    b.commit();
    let a_updates = a.export(ExportMode::all_updates()).expect("export a");
    let b_updates = b.export(ExportMode::all_updates()).expect("export b");
    a.import(&b_updates).expect("a imports b");
    b.import(&a_updates).expect("b imports a");
    a.commit();
    b.commit();
}

/// Forks `base` into an independent replica that shares the same history.
fn fork(base: &LoroDoc) -> LoroDoc {
    base.commit();
    base.fork()
}

// ── Convergence: concurrent edits to the same block ───────────────────────────

#[test]
fn concurrent_inserts_same_block_converge() {
    let a = document_to_loro(&doc_with_paras(1)).expect("to loro");
    let b = fork(&a);

    // Both peers prepend different text to the same block, concurrently.
    insert_text(&a, 0, 0, "AAA").expect("insert a");
    insert_text(&b, 0, 0, "BBB").expect("insert b");

    sync(&a, &b);

    // Convergence: identical resolved state on both replicas.
    assert_eq!(
        a.get_deep_value(),
        b.get_deep_value(),
        "replicas must converge after exchanging concurrent inserts"
    );
    // Both insertions survive (CRDT keeps both, never silently drops one).
    let text = get_block_text(&a, 0);
    assert!(text.contains("AAA"), "lost peer A's insert: {text:?}");
    assert!(text.contains("BBB"), "lost peer B's insert: {text:?}");
    assert!(text.contains("Block0"), "lost the base text: {text:?}");
}

#[test]
fn concurrent_insert_and_delete_converge() {
    let a = document_to_loro(&doc_with_paras(1)).expect("to loro"); // "Block0"
    let b = fork(&a);

    // A inserts at the end; B deletes the leading "Block" (5 bytes).
    insert_text(&a, 0, "Block0".len(), "!").expect("insert a");
    delete_text(&b, 0, 0, "Block".len()).expect("delete b");

    sync(&a, &b);

    assert_eq!(a.get_deep_value(), b.get_deep_value());
    // A's "!" and B's deletion compose: "Block0" -> "0" -> "0!".
    assert_eq!(get_block_text(&a, 0), "0!");
}

// ── Convergence: concurrent edits to different blocks ─────────────────────────

#[test]
fn concurrent_inserts_different_blocks_converge() {
    let a = document_to_loro(&doc_with_paras(2)).expect("to loro");
    let b = fork(&a);

    insert_text(&a, 0, 0, "X").expect("edit block 0 on a");
    insert_text(&b, 1, 0, "Y").expect("edit block 1 on b");

    sync(&a, &b);

    assert_eq!(a.get_deep_value(), b.get_deep_value());
    assert_eq!(get_block_text(&a, 0), "XBlock0");
    assert_eq!(get_block_text(&a, 1), "YBlock1");
}

// ── Convergence: concurrent structural + text edits ───────────────────────────

#[test]
fn concurrent_split_and_text_edit_converge() {
    let a = document_to_loro(&doc_with_paras(2)).expect("to loro");
    let b = fork(&a);

    // A splits block 0 in the middle ("Block" | "0"); B appends to block 1.
    split_block(&a, 0, "Block".len()).expect("split on a");
    insert_text(&b, 1, "Block1".len(), "Z").expect("append on b");

    sync(&a, &b);

    assert_eq!(
        a.get_deep_value(),
        b.get_deep_value(),
        "structural split must converge with a concurrent text edit"
    );
    // A's split added a block; B's edit landed on the (formerly) second block.
    let doc = loro_to_document(&a).expect("from loro");
    assert_eq!(doc.sections[0].blocks.len(), 3);
}

#[test]
fn concurrent_merge_and_text_edit_converge() {
    let a = document_to_loro(&doc_with_paras(3)).expect("to loro");
    let b = fork(&a);

    // A merges block 1 into block 0; B edits block 2 (untouched by the merge).
    merge_block(&a, 1).expect("merge on a");
    insert_text(&b, 2, 0, "Q").expect("edit on b");

    sync(&a, &b);

    assert_eq!(a.get_deep_value(), b.get_deep_value());
    let doc = loro_to_document(&a).expect("from loro");
    assert_eq!(doc.sections[0].blocks.len(), 2);
    assert_eq!(get_block_text(&a, 0), "Block0Block1");
}

// ── Three-way convergence (order independence) ────────────────────────────────

#[test]
fn three_replicas_converge_regardless_of_merge_order() {
    let a = document_to_loro(&doc_with_paras(1)).expect("to loro");
    let b = fork(&a);
    let c = fork(&a);

    insert_text(&a, 0, 0, "1").expect("a");
    insert_text(&b, 0, 0, "2").expect("b");
    insert_text(&c, 0, 0, "3").expect("c");

    // Merge in a deliberately lopsided order: b<-c, a<-b, c<-a, then settle.
    sync(&b, &c);
    sync(&a, &b);
    sync(&c, &a);
    sync(&a, &c);

    assert_eq!(a.get_deep_value(), b.get_deep_value());
    assert_eq!(b.get_deep_value(), c.get_deep_value());
    let text = get_block_text(&a, 0);
    for needle in ["1", "2", "3", "Block0"] {
        assert!(text.contains(needle), "lost {needle:?} in {text:?}");
    }
}

// ── Undo / redo ───────────────────────────────────────────────────────────────

#[test]
fn undo_then_redo_restores_insert() {
    let loro = document_to_loro(&doc_with_paras(1)).expect("to loro");
    let mut um = UndoManager::new(&loro);

    insert_text(&loro, 0, 0, "typed ").expect("insert");
    loro.commit();
    um.record_new_checkpoint().expect("checkpoint");
    assert_eq!(get_block_text(&loro, 0), "typed Block0");

    assert!(
        um.can_undo(),
        "an edit was recorded, so undo must be available"
    );
    um.undo().expect("undo");
    loro.commit();
    assert_eq!(
        get_block_text(&loro, 0),
        "Block0",
        "undo must revert the insert"
    );

    assert!(um.can_redo(), "after an undo, redo must be available");
    um.redo().expect("redo");
    loro.commit();
    assert_eq!(
        get_block_text(&loro, 0),
        "typed Block0",
        "redo must re-apply the insert"
    );
}

// ── Pathological / edge states ────────────────────────────────────────────────

#[test]
fn snapshot_import_into_fresh_doc_preserves_state() {
    let src = document_to_loro(&doc_with_paras(2)).expect("to loro");
    insert_text(&src, 0, 0, "edited ").expect("insert");
    src.commit();

    // Round-trip through a binary snapshot into a brand-new, empty LoroDoc.
    let snapshot = src.export(ExportMode::snapshot()).expect("snapshot");
    let fresh = LoroDoc::new();
    fresh.import(&snapshot).expect("import snapshot");

    assert_eq!(
        src.get_deep_value(),
        fresh.get_deep_value(),
        "a snapshot imported into a fresh doc must reproduce the state exactly"
    );
    assert_eq!(get_block_text(&fresh, 0), "edited Block0");
}

#[test]
fn fork_of_empty_document_converges() {
    // Document::new() has one empty section with no blocks — the degenerate
    // starting point. Forking and editing it must still converge.
    let a = document_to_loro(&Document::new()).expect("to loro");
    let b = fork(&a);

    // Neither peer can edit a non-existent block; assert the helper reports the
    // empty state rather than panicking, then converge the (no-op) histories.
    assert_eq!(get_block_text(&a, 0), "");
    assert!(
        insert_text(&b, 0, 0, "x").is_err(),
        "no block 0 to edit yet"
    );

    sync(&a, &b);
    assert_eq!(a.get_deep_value(), b.get_deep_value());
}

#[test]
fn idempotent_reimport_is_a_noop() {
    let a = document_to_loro(&doc_with_paras(1)).expect("to loro");
    let b = fork(&a);
    insert_text(&b, 0, 0, "once ").expect("insert");
    sync(&a, &b);

    let before = a.get_deep_value();
    // Re-importing the same updates a second time must not duplicate ops.
    let again = b.export(ExportMode::all_updates()).expect("re-export");
    a.import(&again).expect("re-import");
    a.commit();

    assert_eq!(before, a.get_deep_value(), "re-import must be idempotent");
    assert_eq!(get_block_text(&a, 0), "once Block0");
}
