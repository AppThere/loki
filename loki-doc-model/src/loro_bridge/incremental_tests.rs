// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`super::IncrementalReader`].
//!
//! The governing invariant: after any mutation, an incremental `update` must
//! produce a `Document` byte-identical to a full `loro_to_document` rebuild.
//! Each test drives a real mutation through the CRDT, then asserts equality
//! against the full rebuild via `Debug` (which captures all content).

use super::IncrementalReader;
use crate::content::block::Block;
use crate::content::inline::Inline;
use crate::document::Document;
use crate::layout::section::Section;
use crate::loro_bridge::{document_to_loro, loro_to_document};
use crate::loro_mutation::{insert_text, mark_text, merge_block, set_block_style, split_block};
use crate::loro_schema::MARK_BOLD;
use loro::LoroValue;

/// Builds a single-section document of `n` simple paragraphs.
fn doc_with_paras(n: usize) -> Document {
    let blocks: Vec<Block> = (0..n)
        .map(|i| Block::Para(vec![Inline::Str(format!("paragraph number {i}"))]))
        .collect();
    let section = Section::with_layout_and_blocks(Default::default(), blocks);
    let mut doc = Document::new();
    doc.sections = vec![section];
    doc
}

/// Asserts the reader's current document equals a full rebuild of `loro`.
fn assert_matches_full(reader_doc: &Document, loro: &loro::LoroDoc) {
    let full = loro_to_document(loro).expect("full rebuild");
    assert_eq!(
        format!("{reader_doc:?}"),
        format!("{full:?}"),
        "incremental result diverged from full rebuild"
    );
}

#[test]
fn text_insert_matches_full_rebuild() {
    let loro = document_to_loro(&doc_with_paras(6)).expect("to loro");
    let mut reader = IncrementalReader::seed(&loro).expect("seed");

    // Type into the middle paragraph.
    insert_text(&loro, 3, 0, "X").expect("insert");
    let derived = reader.update(&loro).expect("update").clone();

    assert_matches_full(&derived, &loro);
    // The edited block actually reflects the insert.
    if let Some(Block::Para(inlines)) = derived.sections[0].blocks.get(3) {
        assert!(
            matches!(inlines.first(), Some(Inline::Str(s)) if s.starts_with('X')),
            "edited paragraph should start with the inserted char"
        );
    } else {
        panic!("expected Para at index 3");
    }
}

#[test]
fn repeated_text_edits_stay_consistent() {
    let loro = document_to_loro(&doc_with_paras(8)).expect("to loro");
    let mut reader = IncrementalReader::seed(&loro).expect("seed");

    // Several edits across different blocks, each verified against full rebuild.
    for (block, ch) in [(0usize, "a"), (7, "b"), (3, "c"), (3, "d")] {
        insert_text(&loro, block, 0, ch).expect("insert");
        let derived = reader.update(&loro).expect("update").clone();
        assert_matches_full(&derived, &loro);
    }
}

#[test]
fn mark_edit_matches_full_rebuild() {
    let loro = document_to_loro(&doc_with_paras(4)).expect("to loro");
    let mut reader = IncrementalReader::seed(&loro).expect("seed");

    // Bold a range of the second paragraph — a mark on the text container.
    mark_text(
        &loro,
        1,
        0,
        9,
        MARK_BOLD,
        LoroValue::from("true".to_string()),
    )
    .expect("mark");
    let derived = reader.update(&loro).expect("update").clone();

    assert_matches_full(&derived, &loro);
}

#[test]
fn block_style_change_matches_full_rebuild() {
    let loro = document_to_loro(&doc_with_paras(4)).expect("to loro");
    let mut reader = IncrementalReader::seed(&loro).expect("seed");

    // Changing a block's style edits the block map (not the text) — still a
    // descendant of the blocks list, so it maps to a single dirty block.
    set_block_style(&loro, 2, "Heading1").expect("set style");
    let derived = reader.update(&loro).expect("update").clone();

    assert_matches_full(&derived, &loro);
}

#[test]
fn split_block_falls_back_and_matches_full_rebuild() {
    let loro = document_to_loro(&doc_with_paras(4)).expect("to loro");
    let mut reader = IncrementalReader::seed(&loro).expect("seed");

    // A split inserts a new block — structural — and must fall back to a full
    // rebuild, still producing the correct document.
    split_block(&loro, 1, 4).expect("split");
    let derived = reader.update(&loro).expect("update").clone();

    assert_eq!(
        derived.sections[0].blocks.len(),
        5,
        "split should yield one more block"
    );
    assert_matches_full(&derived, &loro);
}

#[test]
fn merge_block_falls_back_and_matches_full_rebuild() {
    let loro = document_to_loro(&doc_with_paras(4)).expect("to loro");
    let mut reader = IncrementalReader::seed(&loro).expect("seed");

    merge_block(&loro, 2).expect("merge");
    let derived = reader.update(&loro).expect("update").clone();

    assert_eq!(
        derived.sections[0].blocks.len(),
        3,
        "merge should remove one block"
    );
    assert_matches_full(&derived, &loro);
}

#[test]
fn structural_then_text_edit_recovers() {
    // After a structural fallback, a subsequent text edit must still be correct
    // (the reader's version/cached state recovered cleanly).
    let loro = document_to_loro(&doc_with_paras(5)).expect("to loro");
    let mut reader = IncrementalReader::seed(&loro).expect("seed");

    split_block(&loro, 0, 4).expect("split");
    let _ = reader.update(&loro).expect("update after split");

    insert_text(&loro, 4, 0, "Z").expect("insert");
    let derived = reader.update(&loro).expect("update after insert").clone();
    assert_matches_full(&derived, &loro);
}

#[test]
fn no_mutation_returns_cached() {
    let loro = document_to_loro(&doc_with_paras(3)).expect("to loro");
    let mut reader = IncrementalReader::seed(&loro).expect("seed");
    let derived = reader.update(&loro).expect("update").clone();
    assert_matches_full(&derived, &loro);
}

// ── Fast-path engagement (not just correctness) ────────────────────────────────

/// Builds a document of several sections, each with the given number of simple
/// paragraphs.
fn doc_with_sections(section_sizes: &[usize]) -> Document {
    let sections = section_sizes
        .iter()
        .enumerate()
        .map(|(s, &n)| {
            let blocks: Vec<Block> = (0..n)
                .map(|i| Block::Para(vec![Inline::Str(format!("s{s} paragraph {i}"))]))
                .collect();
            Section::with_layout_and_blocks(Default::default(), blocks)
        })
        .collect();
    let mut doc = Document::new();
    doc.sections = sections;
    doc
}

/// Section-aware analogue of [`insert_text`] (which only targets section 0):
/// navigates to block `block` of section `sec` and inserts `text` at `offset`.
/// Used to simulate a multi-section / remote edit that reaches a non-zero
/// section, which the production mutation helpers do not currently produce.
fn insert_in_section(loro: &loro::LoroDoc, sec: usize, block: usize, offset: usize, text: &str) {
    use crate::loro_schema::{KEY_BLOCKS, KEY_CONTENT, KEY_SECTIONS};
    let content = loro
        .get_list(KEY_SECTIONS)
        .get(sec)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .and_then(|m| m.get(KEY_BLOCKS))
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_movable_list().ok())
        .and_then(|list| list.get(block))
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .and_then(|m| m.get(KEY_CONTENT))
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_text().ok())
        .expect("navigate to section/block text");
    content.insert_utf8(offset, text).expect("insert");
}

#[test]
fn text_edit_uses_incremental_fast_path() {
    let loro = document_to_loro(&doc_with_paras(6)).expect("to loro");
    let mut reader = IncrementalReader::seed(&loro).expect("seed");

    insert_text(&loro, 3, 0, "X").expect("insert");
    reader.update(&loro).expect("update");

    assert!(
        reader.last_update_was_incremental(),
        "a plain text edit must take the block-local fast path"
    );
}

#[test]
fn structural_edit_reports_non_incremental() {
    let loro = document_to_loro(&doc_with_paras(4)).expect("to loro");
    let mut reader = IncrementalReader::seed(&loro).expect("seed");

    split_block(&loro, 1, 4).expect("split");
    reader.update(&loro).expect("update");

    assert!(
        !reader.last_update_was_incremental(),
        "a block split changes the block count and must fall back to a full rebuild"
    );
}

#[test]
fn edit_in_second_section_is_incremental() {
    // A multi-section document (or a remote peer's edit) reaching section 1 must
    // patch that section incrementally rather than rebuilding the whole document.
    let loro = document_to_loro(&doc_with_sections(&[3, 3])).expect("to loro");
    let mut reader = IncrementalReader::seed(&loro).expect("seed");

    insert_in_section(&loro, 1, 2, 0, "Z");
    let derived = reader.update(&loro).expect("update").clone();

    assert!(
        reader.last_update_was_incremental(),
        "an edit confined to a section-1 block must take the fast path"
    );
    assert_matches_full(&derived, &loro);
    if let Some(Block::Para(inlines)) = derived.sections[1].blocks.get(2) {
        assert!(
            matches!(inlines.first(), Some(Inline::Str(s)) if s.starts_with('Z')),
            "the edited section-1 paragraph should reflect the insert"
        );
    } else {
        panic!("expected Para at section 1, block 2");
    }
}

#[test]
fn edits_across_sections_stay_incremental_and_consistent() {
    let loro = document_to_loro(&doc_with_sections(&[2, 2, 2])).expect("to loro");
    let mut reader = IncrementalReader::seed(&loro).expect("seed");

    for (sec, block, ch) in [(0usize, 1usize, "a"), (2, 0, "b"), (1, 1, "c")] {
        insert_in_section(&loro, sec, block, 0, ch);
        let derived = reader.update(&loro).expect("update").clone();
        assert!(
            reader.last_update_was_incremental(),
            "edit to section {sec} block {block} should be incremental"
        );
        assert_matches_full(&derived, &loro);
    }
}
