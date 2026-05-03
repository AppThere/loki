// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for `loki_doc_model::loro_mutation`.
//!
//! These tests verify that `insert_text`, `delete_text`, and `get_block_text`
//! operate correctly against a `LoroDoc` populated by `document_to_loro`.

use loki_doc_model::{
    content::block::{Block, StyledParagraph},
    content::inline::Inline,
    delete_text, get_block_text, insert_text,
    layout::section::Section,
    loro_bridge::document_to_loro,
    style::StyleId,
    Document, MutationError,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a minimal `Document` with `paragraphs` as the first section's blocks.
///
/// Each paragraph is represented as a `Block::StyledPara` with a single
/// `Inline::Str` run.  This matches the simplest case handled by
/// `document_to_loro` without any special inline types.
fn make_doc_with_paragraphs(paragraphs: &[&str]) -> Document {
    let mut doc = Document::new();
    let mut section = Section::new();
    for text in paragraphs {
        section.blocks.push(Block::StyledPara(StyledParagraph {
            style_id: Some(StyleId::new("Normal")),
            direct_para_props: None,
            direct_char_props: None,
            inlines: vec![Inline::Str((*text).into())],
            attr: loki_doc_model::NodeAttr::default(),
        }));
    }
    doc.sections.clear();
    doc.sections.push(section);
    doc
}

// ── Mutation tests ────────────────────────────────────────────────────────────

#[test]
fn insert_text_at_offset_0_prepends() {
    let doc = make_doc_with_paragraphs(&["hello"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    insert_text(&ldoc, 0, 0, "X").expect("insert succeeded");

    let text = get_block_text(&ldoc, 0);
    assert!(text.starts_with('X'), "expected text to start with 'X', got: {text:?}");
}

#[test]
fn insert_text_at_end_appends() {
    let doc = make_doc_with_paragraphs(&["hello"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    // "hello" is 5 bytes; inserting at offset 5 should append.
    let initial_len = get_block_text(&ldoc, 0).len();
    insert_text(&ldoc, 0, initial_len, "!").expect("insert at end succeeded");

    let text = get_block_text(&ldoc, 0);
    assert!(text.ends_with('!'), "expected text to end with '!', got: {text:?}");
}

#[test]
fn insert_text_in_middle_inserts_at_correct_position() {
    let doc = make_doc_with_paragraphs(&["hello"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    // Insert "XYZ" at byte offset 2 → "heXYZllo"
    insert_text(&ldoc, 0, 2, "XYZ").expect("insert in middle succeeded");

    let text = get_block_text(&ldoc, 0);
    assert_eq!(text.get(2..5), Some("XYZ"), "mid-string insert mismatch: {text:?}");
}

#[test]
fn delete_text_removes_correct_range() {
    let doc = make_doc_with_paragraphs(&["hello world"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    // Delete " world" (bytes 5..11, len 6) → "hello"
    delete_text(&ldoc, 0, 5, 6).expect("delete succeeded");

    let text = get_block_text(&ldoc, 0);
    assert_eq!(text, "hello", "after delete got: {text:?}");
}

#[test]
fn delete_text_len_zero_is_noop() {
    let doc = make_doc_with_paragraphs(&["hello"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    delete_text(&ldoc, 0, 2, 0).expect("zero-len delete is Ok");

    let text = get_block_text(&ldoc, 0);
    assert_eq!(text, "hello", "zero-len delete must not change text");
}

#[test]
fn out_of_range_block_index_returns_error() {
    let doc = make_doc_with_paragraphs(&["only one paragraph"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    // Block index 99 is out of range.
    let result = insert_text(&ldoc, 99, 0, "x");
    assert!(
        matches!(result, Err(MutationError::BlockIndexOutOfRange(99))),
        "expected BlockIndexOutOfRange(99), got: {result:?}"
    );
}

#[test]
fn get_block_text_returns_current_text_after_mutation() {
    let doc = make_doc_with_paragraphs(&["abc"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    insert_text(&ldoc, 0, 3, "def").expect("insert succeeded");

    let text = get_block_text(&ldoc, 0);
    assert_eq!(text, "abcdef", "text after mutation: {text:?}");
}

// ── Integration smoke test ────────────────────────────────────────────────────

/// Full round-trip: `document_to_loro` → `insert_text` → `loro_to_document`.
///
/// Verifies that a text mutation applied to the CRDT is correctly reflected
/// when the document snapshot is re-derived for layout.
#[test]
fn round_trip_insert_text_visible_in_loro_to_document() {
    use loki_doc_model::loro_bridge::loro_to_document;

    let doc = make_doc_with_paragraphs(&["hello"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    // Insert "X" at the start.
    insert_text(&ldoc, 0, 0, "X").expect("insert succeeded");

    // Re-derive document snapshot.
    let derived = loro_to_document(&ldoc).expect("loro_to_document succeeded");

    // Paragraph 0's first inline should now start with "X".
    let first_section = derived.sections.first().expect("at least one section");
    let first_block = first_section.blocks.first().expect("at least one block");

    let inline_text = match first_block {
        Block::StyledPara(sp) => sp
            .inlines
            .iter()
            .filter_map(|i| if let Inline::Str(s) = i { Some(s.as_str()) } else { None })
            .collect::<Vec<_>>()
            .join(""),
        other => panic!("expected StyledPara, got: {other:?}"),
    };

    assert!(
        inline_text.starts_with('X'),
        "derived paragraph text should start with 'X', got: {inline_text:?}"
    );
}
