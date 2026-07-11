// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Integration tests for `loki_doc_model::loro_mutation`.
//!
//! These tests verify that `insert_text`, `delete_text`, `get_block_text`,
//! `split_block`, and `merge_block` operate correctly against a `LoroDoc`
//! populated by `document_to_loro`.

use loki_doc_model::{
    Document, MutationError, NodeAttr, clear_block_list,
    content::block::{Block, StyledParagraph},
    content::inline::Inline,
    delete_block, delete_text, get_block_list_id, get_block_text, insert_text,
    layout::section::Section,
    loro_bridge::document_to_loro,
    merge_block, split_block,
    style::{
        StyleId,
        list_style::ListId,
        props::{CharProps, ParaProps},
    },
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
            attr: NodeAttr::default(),
        }));
    }
    doc.sections.clear();
    doc.sections.push(section);
    doc
}

/// Build a `Document` whose first section contains a single heading block
/// of the given `level` (1–6) with `text` as its content.
fn make_doc_with_heading(level: u8, text: &str) -> Document {
    let mut doc = Document::new();
    let mut section = Section::new();
    section.blocks.push(Block::Heading(
        level,
        NodeAttr::default(),
        vec![Inline::Str(text.into())],
    ));
    doc.sections.clear();
    doc.sections.push(section);
    doc
}

/// Build a `Document` with a single `StyledPara` that has the supplied
/// `direct_para_props` set.
fn make_doc_with_para_props(text: &str, para_props: ParaProps) -> Document {
    let mut doc = Document::new();
    let mut section = Section::new();
    section.blocks.push(Block::StyledPara(StyledParagraph {
        style_id: Some(StyleId::new("Normal")),
        direct_para_props: Some(Box::new(para_props)),
        direct_char_props: None,
        inlines: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    }));
    doc.sections.clear();
    doc.sections.push(section);
    doc
}

/// Build a `Document` with a single `StyledPara` that has the supplied
/// `direct_char_props` set.
fn make_doc_with_char_props(text: &str, char_props: CharProps) -> Document {
    let mut doc = Document::new();
    let mut section = Section::new();
    section.blocks.push(Block::StyledPara(StyledParagraph {
        style_id: Some(StyleId::new("Normal")),
        direct_para_props: None,
        direct_char_props: Some(Box::new(char_props)),
        inlines: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    }));
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
    assert!(
        text.starts_with('X'),
        "expected text to start with 'X', got: {text:?}"
    );
}

#[test]
fn insert_text_at_end_appends() {
    let doc = make_doc_with_paragraphs(&["hello"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    // "hello" is 5 bytes; inserting at offset 5 should append.
    let initial_len = get_block_text(&ldoc, 0).len();
    insert_text(&ldoc, 0, initial_len, "!").expect("insert at end succeeded");

    let text = get_block_text(&ldoc, 0);
    assert!(
        text.ends_with('!'),
        "expected text to end with '!', got: {text:?}"
    );
}

#[test]
fn insert_text_in_middle_inserts_at_correct_position() {
    let doc = make_doc_with_paragraphs(&["hello"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    // Insert "XYZ" at byte offset 2 → "heXYZllo"
    insert_text(&ldoc, 0, 2, "XYZ").expect("insert in middle succeeded");

    let text = get_block_text(&ldoc, 0);
    assert_eq!(
        text.get(2..5),
        Some("XYZ"),
        "mid-string insert mismatch: {text:?}"
    );
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
            .filter_map(|i| {
                if let Inline::Str(s) = i {
                    Some(s.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(""),
        other => panic!("expected StyledPara, got: {other:?}"),
    };

    assert!(
        inline_text.starts_with('X'),
        "derived paragraph text should start with 'X', got: {inline_text:?}"
    );
}

// ── split_block tests ─────────────────────────────────────────────────────────

#[test]
fn split_block_in_middle_divides_text() {
    let doc = make_doc_with_paragraphs(&["hello world"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    // Split after "hello" (byte offset 5).
    split_block(&ldoc, 0, 5).expect("split succeeded");

    assert_eq!(
        get_block_text(&ldoc, 0),
        "hello",
        "block 0 should be 'hello'"
    );
    assert_eq!(
        get_block_text(&ldoc, 1),
        " world",
        "block 1 should be ' world'"
    );
}

#[test]
fn split_block_at_start_yields_empty_first_block() {
    let doc = make_doc_with_paragraphs(&["hello"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    split_block(&ldoc, 0, 0).expect("split at start succeeded");

    assert_eq!(get_block_text(&ldoc, 0), "", "block 0 should be empty");
    assert_eq!(
        get_block_text(&ldoc, 1),
        "hello",
        "block 1 should carry full text"
    );
}

#[test]
fn split_block_at_end_yields_empty_second_block() {
    let doc = make_doc_with_paragraphs(&["hello"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    split_block(&ldoc, 0, 5).expect("split at end succeeded");

    assert_eq!(
        get_block_text(&ldoc, 0),
        "hello",
        "block 0 should be full text"
    );
    assert_eq!(get_block_text(&ldoc, 1), "", "block 1 should be empty");
}

#[test]
fn split_block_unicode_boundary_is_respected() {
    // "café" in UTF-8: 'c'=1, 'a'=1, 'f'=1, 'é'=2 → total 5 bytes.
    // Valid split at offset 3 ("caf" | "é").
    let doc = make_doc_with_paragraphs(&["café"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    split_block(&ldoc, 0, 3).expect("split at valid unicode boundary succeeded");

    assert_eq!(get_block_text(&ldoc, 0), "caf");
    assert_eq!(get_block_text(&ldoc, 1), "é");
}

#[test]
fn split_block_invalid_byte_offset_returns_error() {
    // "café" — offset 4 is inside the 'é' multibyte sequence (not a char boundary).
    let doc = make_doc_with_paragraphs(&["café"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    let result = split_block(&ldoc, 0, 4);
    assert!(
        matches!(result, Err(MutationError::InvalidByteOffset { offset: 4 })),
        "expected InvalidByteOffset(4), got: {result:?}"
    );
}

#[test]
fn split_block_out_of_range_returns_error() {
    let doc = make_doc_with_paragraphs(&["only one block"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    let result = split_block(&ldoc, 99, 0);
    assert!(
        matches!(result, Err(MutationError::BlockIndexOutOfRange(99))),
        "expected BlockIndexOutOfRange(99), got: {result:?}"
    );
}

#[test]
fn split_block_second_block_inherits_block_type() {
    let doc = make_doc_with_paragraphs(&["paragraph text"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    split_block(&ldoc, 0, 9).expect("split succeeded");

    // Both blocks should re-derive as StyledPara (type was copied).
    let derived =
        loki_doc_model::loro_bridge::loro_to_document(&ldoc).expect("loro_to_document succeeded");
    let section = derived.sections.first().expect("section exists");
    assert_eq!(
        section.blocks.len(),
        2,
        "should have two blocks after split"
    );
    assert!(
        matches!(section.blocks[0], Block::StyledPara(_)),
        "block 0 should be StyledPara"
    );
    assert!(
        matches!(section.blocks[1], Block::StyledPara(_)),
        "block 1 should be StyledPara (inherited type)"
    );
}

// ── merge_block tests ─────────────────────────────────────────────────────────

#[test]
fn merge_block_concatenates_text() {
    let doc = make_doc_with_paragraphs(&["hello", " world"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    merge_block(&ldoc, 1).expect("merge succeeded");

    assert_eq!(
        get_block_text(&ldoc, 0),
        "hello world",
        "merged text mismatch"
    );
}

#[test]
fn merge_block_returns_correct_merged_offset() {
    let doc = make_doc_with_paragraphs(&["hello", " world"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    let offset = merge_block(&ldoc, 1).expect("merge succeeded");

    // merged_offset should equal the byte length of "hello" = 5.
    assert_eq!(offset, 5, "merged_offset should point to the join position");
}

#[test]
fn merge_block_at_index_zero_returns_no_previous_block() {
    let doc = make_doc_with_paragraphs(&["only block"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    let result = merge_block(&ldoc, 0);
    assert!(
        matches!(result, Err(MutationError::NoPreviousBlock)),
        "expected NoPreviousBlock, got: {result:?}"
    );
}

#[test]
fn merge_block_out_of_range_returns_error() {
    let doc = make_doc_with_paragraphs(&["only block"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    let result = merge_block(&ldoc, 99);
    assert!(
        matches!(result, Err(MutationError::BlockIndexOutOfRange(99))),
        "expected BlockIndexOutOfRange(99), got: {result:?}"
    );
}

#[test]
fn merge_removes_the_second_block() {
    let doc = make_doc_with_paragraphs(&["first", "second"]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    merge_block(&ldoc, 1).expect("merge succeeded");

    let derived =
        loki_doc_model::loro_bridge::loro_to_document(&ldoc).expect("loro_to_document succeeded");
    let section = derived.sections.first().expect("section exists");
    assert_eq!(
        section.blocks.len(),
        1,
        "only one block should remain after merge"
    );
}

// ── split/merge round-trip ────────────────────────────────────────────────────

#[test]
fn split_then_merge_round_trips_text() {
    let original = "hello world";
    let doc = make_doc_with_paragraphs(&[original]);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    // Split at "hello " | "world".
    split_block(&ldoc, 0, 6).expect("split succeeded");
    assert_eq!(get_block_text(&ldoc, 0), "hello ");
    assert_eq!(get_block_text(&ldoc, 1), "world");

    // Merge back.
    let offset = merge_block(&ldoc, 1).expect("merge succeeded");
    assert_eq!(offset, 6, "merged_offset should equal split point");
    assert_eq!(
        get_block_text(&ldoc, 0),
        original,
        "round-trip text mismatch"
    );
}

// ── split_block style-preservation tests ─────────────────────────────────────

#[test]
fn split_heading_block_preserves_heading_level() {
    // Level-2 heading "Hello World" — split after "Hello" (5 bytes).
    let doc = make_doc_with_heading(2, "Hello World");
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    split_block(&ldoc, 0, 5).expect("split succeeded");

    let derived =
        loki_doc_model::loro_bridge::loro_to_document(&ldoc).expect("loro_to_document succeeded");
    let section = derived.sections.first().expect("section exists");
    assert_eq!(section.blocks.len(), 2, "two blocks after split");

    match &section.blocks[0] {
        Block::Heading(lvl, _, _) => assert_eq!(*lvl, 2, "block 0 must be heading level 2"),
        other => panic!("block 0 should be Heading, got: {other:?}"),
    }
    match &section.blocks[1] {
        Block::Heading(lvl, _, _) => assert_eq!(*lvl, 2, "block 1 must inherit heading level 2"),
        other => panic!("block 1 should be Heading (level inherited), got: {other:?}"),
    }
}

#[test]
fn split_heading_level_1_is_preserved() {
    let doc = make_doc_with_heading(1, "Title");
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    split_block(&ldoc, 0, 0).expect("split at start succeeded");

    let derived =
        loki_doc_model::loro_bridge::loro_to_document(&ldoc).expect("loro_to_document succeeded");
    let section = derived.sections.first().expect("section exists");
    for (i, block) in section.blocks.iter().enumerate() {
        match block {
            Block::Heading(lvl, _, _) => {
                assert_eq!(*lvl, 1, "block {i} must be heading level 1 after split")
            }
            other => panic!("block {i} should be Heading, got: {other:?}"),
        }
    }
}

#[test]
fn split_block_with_para_props_inherits_props() {
    use loki_doc_model::style::props::para_props::ParagraphAlignment;

    let para_props = ParaProps {
        alignment: Some(ParagraphAlignment::Center),
        ..Default::default()
    };

    let doc = make_doc_with_para_props("centered text", para_props);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    split_block(&ldoc, 0, 8).expect("split succeeded");

    let derived =
        loki_doc_model::loro_bridge::loro_to_document(&ldoc).expect("loro_to_document succeeded");
    let section = derived.sections.first().expect("section exists");
    assert_eq!(section.blocks.len(), 2, "two blocks after split");

    for (i, block) in section.blocks.iter().enumerate() {
        match block {
            Block::StyledPara(sp) => {
                let alignment = sp
                    .direct_para_props
                    .as_ref()
                    .and_then(|p| p.alignment.as_ref());
                assert_eq!(
                    alignment,
                    Some(&ParagraphAlignment::Center),
                    "block {i} must inherit Center alignment"
                );
            }
            other => panic!("block {i} should be StyledPara, got: {other:?}"),
        }
    }
}

#[test]
fn split_block_new_block_props_are_independent() {
    // Verify that the two `para_props` LoroMaps after a split are separate
    // containers — mutating block 1's text must not change block 0's text.
    use loki_doc_model::style::props::para_props::ParagraphAlignment;

    let para_props = ParaProps {
        alignment: Some(ParagraphAlignment::Right),
        ..Default::default()
    };

    let doc = make_doc_with_para_props("right aligned paragraph", para_props);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    split_block(&ldoc, 0, 5).expect("split succeeded");
    assert_eq!(
        get_block_text(&ldoc, 0),
        "right",
        "block 0 text after split"
    );
    assert_eq!(
        get_block_text(&ldoc, 1),
        " aligned paragraph",
        "block 1 text after split"
    );

    // Insert text into block 1 only; block 0 must be unaffected.
    loki_doc_model::insert_text(&ldoc, 1, 0, "XXX").expect("insert into block 1 succeeded");
    assert_eq!(
        get_block_text(&ldoc, 0),
        "right",
        "block 0 must be unchanged after block 1 mutation"
    );
    assert_eq!(
        get_block_text(&ldoc, 1),
        "XXX aligned paragraph",
        "block 1 has inserted text"
    );
}

#[test]
fn split_block_with_char_props_inherits_direct_char_props() {
    let char_props = CharProps {
        bold: Some(true),
        ..Default::default()
    };

    let doc = make_doc_with_char_props("bold text here", char_props);
    let ldoc = document_to_loro(&doc).expect("document_to_loro succeeded");

    split_block(&ldoc, 0, 4).expect("split succeeded");

    let derived =
        loki_doc_model::loro_bridge::loro_to_document(&ldoc).expect("loro_to_document succeeded");
    let section = derived.sections.first().expect("section exists");
    assert_eq!(section.blocks.len(), 2, "two blocks after split");

    for (i, block) in section.blocks.iter().enumerate() {
        match block {
            Block::StyledPara(sp) => {
                let bold = sp.direct_char_props.as_ref().and_then(|c| c.bold);
                assert_eq!(
                    bold,
                    Some(true),
                    "block {i} must inherit bold=true from direct_char_props"
                );
            }
            other => panic!("block {i} should be StyledPara, got: {other:?}"),
        }
    }
}

// ── Multi-section editing ──────────────────────────────────────────────────────

/// Build a multi-section `Document`; each inner slice is one section's
/// paragraphs. Editor block indices are global across sections (section 0's
/// blocks occupy `0..a`, section 1's `a..a+b`, and so on).
fn make_doc_with_sections(sections: &[&[&str]]) -> Document {
    let mut doc = Document::new();
    doc.sections.clear();
    for paras in sections {
        let mut section = Section::new();
        for text in *paras {
            section.blocks.push(Block::StyledPara(StyledParagraph {
                style_id: Some(StyleId::new("Normal")),
                direct_para_props: None,
                direct_char_props: None,
                inlines: vec![Inline::Str((*text).into())],
                attr: NodeAttr::default(),
            }));
        }
        doc.sections.push(section);
    }
    doc
}

/// Plain-text content of a paragraph/heading block (for assertions).
fn block_text(block: &Block) -> String {
    let inlines = match block {
        Block::StyledPara(sp) => &sp.inlines,
        Block::Para(inlines) => inlines,
        _ => return String::new(),
    };
    inlines
        .iter()
        .filter_map(|i| match i {
            Inline::Str(s) => Some(s.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn insert_text_targets_the_correct_section() {
    // Global index 2 is the first block of section 1.
    let ldoc = document_to_loro(&make_doc_with_sections(&[&["a0", "a1"], &["b0", "b1"]]))
        .expect("to loro");

    insert_text(&ldoc, 2, 0, "X").expect("insert into section 1");

    assert_eq!(get_block_text(&ldoc, 2), "Xb0", "edit lands in section 1");
    assert_eq!(
        get_block_text(&ldoc, 0),
        "a0",
        "section 0 block 0 untouched"
    );
    assert_eq!(
        get_block_text(&ldoc, 1),
        "a1",
        "section 0 block 1 untouched"
    );

    let doc = loki_doc_model::loro_bridge::loro_to_document(&ldoc).expect("rebuild");
    assert_eq!(doc.sections.len(), 2);
    assert_eq!(block_text(&doc.sections[1].blocks[0]), "Xb0");
    assert_eq!(block_text(&doc.sections[0].blocks[0]), "a0");
}

#[test]
fn split_block_in_second_section_only_affects_that_section() {
    // Global: 0="a0"; 1="b0"; 2="b1".
    let ldoc =
        document_to_loro(&make_doc_with_sections(&[&["a0"], &["b0", "b1"]])).expect("to loro");

    split_block(&ldoc, 1, 1).expect("split b0 -> 'b' + '0'");

    let doc = loki_doc_model::loro_bridge::loro_to_document(&ldoc).expect("rebuild");
    assert_eq!(doc.sections[0].blocks.len(), 1, "section 0 unchanged");
    assert_eq!(doc.sections[1].blocks.len(), 3, "section 1 gained a block");
    assert_eq!(block_text(&doc.sections[1].blocks[0]), "b");
    assert_eq!(block_text(&doc.sections[1].blocks[1]), "0");
    assert_eq!(block_text(&doc.sections[1].blocks[2]), "b1");
}

#[test]
fn merge_within_a_section_works() {
    // Global: 0="a0"; 1="b0"; 2="b1". Merge b1 into b0.
    let ldoc =
        document_to_loro(&make_doc_with_sections(&[&["a0"], &["b0", "b1"]])).expect("to loro");

    let offset = merge_block(&ldoc, 2).expect("merge within section 1");
    assert_eq!(offset, 2, "join offset is the former byte length of 'b0'");

    let doc = loki_doc_model::loro_bridge::loro_to_document(&ldoc).expect("rebuild");
    assert_eq!(doc.sections[0].blocks.len(), 1);
    assert_eq!(doc.sections[1].blocks.len(), 1, "section 1 lost a block");
    assert_eq!(block_text(&doc.sections[1].blocks[0]), "b0b1");
}

#[test]
fn merge_across_a_section_break_is_rejected() {
    // Global index 1 is the first block of section 1, so its predecessor lives
    // in section 0 — a cross-section merge, which is not supported.
    let ldoc = document_to_loro(&make_doc_with_sections(&[&["a0"], &["b0"]])).expect("to loro");

    let err = merge_block(&ldoc, 1).expect_err("cross-section merge must be rejected");
    assert!(
        matches!(err, MutationError::CrossSectionMerge),
        "got {err:?}"
    );

    // Nothing changed.
    let doc = loki_doc_model::loro_bridge::loro_to_document(&ldoc).expect("rebuild");
    assert_eq!(doc.sections[0].blocks.len(), 1);
    assert_eq!(doc.sections[1].blocks.len(), 1);
    assert_eq!(block_text(&doc.sections[1].blocks[0]), "b0");
}

#[test]
fn global_index_past_the_last_section_errors() {
    let ldoc = document_to_loro(&make_doc_with_sections(&[&["a0"], &["b0"]])).expect("to loro");
    // Only global indices 0 and 1 exist (one block per section).
    let err = insert_text(&ldoc, 2, 0, "X").expect_err("index past last block");
    assert!(
        matches!(err, MutationError::BlockIndexOutOfRange(2)),
        "got {err:?}"
    );
}

// ── List paragraph props: read + clear (plan 4b.1 list-exit) ────────────────

/// A `Document` with one list paragraph: `list_id = "L1"`, `list_level = 0`.
fn make_doc_with_list_item(text: &str) -> Document {
    let para_props = ParaProps {
        list_id: Some(ListId::new("L1")),
        list_level: Some(0),
        ..ParaProps::default()
    };
    make_doc_with_para_props(text, para_props)
}

#[test]
fn get_block_list_id_reads_direct_list_membership() {
    let ldoc = document_to_loro(&make_doc_with_list_item("item")).expect("to loro");
    assert_eq!(get_block_list_id(&ldoc, 0).as_deref(), Some("L1"));
}

#[test]
fn get_block_list_id_is_none_for_a_plain_paragraph() {
    let ldoc = document_to_loro(&make_doc_with_paragraphs(&["plain"])).expect("to loro");
    assert_eq!(get_block_list_id(&ldoc, 0), None);
}

#[test]
fn clear_block_list_removes_list_membership() {
    let ldoc = document_to_loro(&make_doc_with_list_item("item")).expect("to loro");
    assert_eq!(
        get_block_list_id(&ldoc, 0).as_deref(),
        Some("L1"),
        "starts a list item"
    );

    clear_block_list(&ldoc, 0).expect("clear ok");

    // The paragraph is no longer a list item and its text is untouched.
    assert_eq!(get_block_list_id(&ldoc, 0), None, "list membership cleared");
    assert_eq!(get_block_text(&ldoc, 0), "item");

    // The list_level prop is gone too, confirmed via a full round-trip.
    let doc = loki_doc_model::loro_bridge::loro_to_document(&ldoc).expect("rebuild");
    let Block::StyledPara(sp) = &doc.sections[0].blocks[0] else {
        panic!("expected StyledPara");
    };
    let props = sp.direct_para_props.as_ref();
    assert!(
        props.is_none_or(|p| p.list_id.is_none() && p.list_level.is_none()),
        "both list props cleared, got {props:?}",
    );
}

#[test]
fn clear_block_list_on_a_plain_paragraph_is_a_noop() {
    let ldoc = document_to_loro(&make_doc_with_paragraphs(&["plain"])).expect("to loro");
    clear_block_list(&ldoc, 0).expect("no-op ok");
    assert_eq!(get_block_text(&ldoc, 0), "plain");
    assert_eq!(get_block_list_id(&ldoc, 0), None);
}

// ── delete_block (contextual Table tab "Delete Table") ──────────────────────

#[test]
fn delete_block_removes_the_addressed_block() {
    let ldoc = document_to_loro(&make_doc_with_paragraphs(&["a", "b", "c"])).expect("to loro");
    delete_block(&ldoc, 1).expect("delete middle block");

    let doc = loki_doc_model::loro_bridge::loro_to_document(&ldoc).expect("rebuild");
    let texts: Vec<String> = doc.sections[0]
        .blocks
        .iter()
        .map(|b| match b {
            Block::StyledPara(sp) => sp
                .inlines
                .iter()
                .filter_map(|i| match i {
                    Inline::Str(s) => Some(s.as_str()),
                    _ => None,
                })
                .collect(),
            _ => String::new(),
        })
        .collect();
    assert_eq!(texts, vec!["a", "c"], "block 'b' removed, order preserved");
}

#[test]
fn delete_block_resolves_across_sections() {
    // Global index 2 is the first block of the second section.
    let ldoc = document_to_loro(&make_doc_with_sections(&[&["a0", "a1"], &["b0", "b1"]]))
        .expect("to loro");
    delete_block(&ldoc, 2).expect("delete b0");

    let doc = loki_doc_model::loro_bridge::loro_to_document(&ldoc).expect("rebuild");
    assert_eq!(doc.sections[0].blocks.len(), 2, "section 0 untouched");
    assert_eq!(doc.sections[1].blocks.len(), 1, "section 1 lost one block");
    assert_eq!(block_text(&doc.sections[1].blocks[0]), "b1");
}

#[test]
fn delete_block_out_of_range_errors() {
    let ldoc = document_to_loro(&make_doc_with_paragraphs(&["only"])).expect("to loro");
    let err = delete_block(&ldoc, 5).expect_err("out of range");
    assert!(
        matches!(err, MutationError::BlockIndexOutOfRange(5)),
        "got {err:?}"
    );
}
