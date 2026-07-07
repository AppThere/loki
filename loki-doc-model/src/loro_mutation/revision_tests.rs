// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for CRDT-level accept / reject of tracked changes.

use super::*;
use crate::content::attr::NodeAttr;
use crate::content::block::Block;
use crate::content::inline::{Inline, StyledRun};
use crate::document::Document;
use crate::layout::page::PageLayout;
use crate::layout::section::Section;
use crate::loro_bridge::{document_to_loro, loro_to_document};
use crate::style::props::char_props::CharProps;
use crate::style::props::revision::{RevisionKind, RevisionMark};

fn tracked(kind: RevisionKind, text: &str) -> Inline {
    Inline::StyledRun(StyledRun {
        style_id: None,
        direct_props: Some(Box::new(CharProps {
            revision: Some(RevisionMark::new(kind).with_author("Ada")),
            ..CharProps::default()
        })),
        content: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    })
}

/// "Keep " + inserted "new" + deleted "old" in a single paragraph.
fn mixed_doc() -> LoroDoc {
    let mut doc = Document::new();
    doc.sections = vec![Section::with_layout_and_blocks(
        PageLayout::default(),
        vec![Block::Para(vec![
            Inline::Str("Keep ".into()),
            tracked(RevisionKind::Insertion, "new"),
            tracked(RevisionKind::Deletion, "old"),
        ])],
    )];
    document_to_loro(&doc).expect("to loro")
}

fn para_text(loro: &LoroDoc) -> String {
    let doc = loro_to_document(loro).expect("rebuild");
    match &doc.sections[0].blocks[0] {
        Block::Para(inlines) => crate::content::toc::inline_plain_text(inlines),
        _ => panic!("expected a paragraph"),
    }
}

#[test]
fn accept_all_keeps_insertions_removes_deletions_and_clears_marks() {
    let loro = mixed_doc();
    let n = accept_reject_all_revisions(&loro, true).expect("accept");
    assert_eq!(n, 2, "two change runs resolved");
    assert_eq!(para_text(&loro), "Keep new");
    // No revision marks remain.
    assert!(!loro_to_document(&loro).unwrap().has_tracked_changes());
}

#[test]
fn reject_all_removes_insertions_keeps_deletions() {
    let loro = mixed_doc();
    let n = accept_reject_all_revisions(&loro, false).expect("reject");
    assert_eq!(n, 2);
    assert_eq!(para_text(&loro), "Keep old");
    assert!(!loro_to_document(&loro).unwrap().has_tracked_changes());
}

#[test]
fn nothing_to_resolve_when_there_are_no_changes() {
    let mut doc = Document::new();
    doc.sections = vec![Section::with_layout_and_blocks(
        PageLayout::default(),
        vec![Block::Para(vec![Inline::Str("plain".into())])],
    )];
    let loro = document_to_loro(&doc).expect("to loro");
    assert_eq!(accept_reject_all_revisions(&loro, true).expect("no-op"), 0);
    assert_eq!(para_text(&loro), "plain");
}

/// A single-paragraph doc holding `text` (untracked) with an empty first para.
fn doc_with_text(text: &str) -> LoroDoc {
    let mut doc = Document::new();
    doc.sections = vec![Section::with_layout_and_blocks(
        PageLayout::default(),
        vec![Block::Para(vec![Inline::Str(text.into())])],
    )];
    document_to_loro(&doc).expect("to loro")
}

#[test]
fn tracked_delete_of_normal_text_strikes_it_through() {
    use crate::content::revision_ops::DeleteAction;
    use crate::loro_mutation::BlockPath;

    let loro = doc_with_text("abc");
    let del = RevisionMark::new(RevisionKind::Deletion).with_author("Ada");
    // Delete the middle grapheme "b" (bytes 1..2) with tracking on.
    let action =
        tracked_grapheme_delete(&loro, &BlockPath::block(0), 1, 2, Some(&del)).expect("mark");
    assert_eq!(action, DeleteAction::MarkDeleted);

    let back = loro_to_document(&loro).expect("rebuild");
    let Block::Para(inlines) = &back.sections[0].blocks[0] else {
        panic!("expected a paragraph");
    };
    // The text is still "abc" (nothing removed) but "b" now carries a deletion.
    assert_eq!(crate::content::toc::inline_plain_text(inlines), "abc");
    assert!(back.has_tracked_changes());
    let struck: String = inlines
        .iter()
        .filter(|i| {
            matches!(i, Inline::StyledRun(r)
            if r.direct_props.as_ref().and_then(|p| p.revision.as_ref())
                .is_some_and(|m| m.kind == RevisionKind::Deletion))
        })
        .map(|i| match i {
            Inline::StyledRun(r) => crate::content::toc::inline_plain_text(&r.content),
            _ => String::new(),
        })
        .collect();
    assert_eq!(struck, "b");
}

#[test]
fn tracked_delete_off_hard_deletes() {
    use crate::content::revision_ops::DeleteAction;
    use crate::loro_mutation::BlockPath;

    let loro = doc_with_text("abc");
    // Tracking off (deletion = None): the grapheme is removed.
    let action = tracked_grapheme_delete(&loro, &BlockPath::block(0), 1, 2, None).expect("delete");
    assert_eq!(action, DeleteAction::HardDelete);
    let back = loro_to_document(&loro).expect("rebuild");
    let Block::Para(inlines) = &back.sections[0].blocks[0] else {
        panic!("expected a paragraph");
    };
    assert_eq!(crate::content::toc::inline_plain_text(inlines), "ac");
}

#[test]
fn tracked_delete_removes_own_insertion_and_skips_deletion() {
    use crate::content::revision_ops::DeleteAction;
    use crate::loro_mutation::{BlockPath, insert_text_tracked_at};

    let loro = doc_with_text("x");
    let path = BlockPath::block(0);
    let ins = RevisionMark::new(RevisionKind::Insertion).with_author("Ada");
    let del = RevisionMark::new(RevisionKind::Deletion).with_author("Ada");
    // Insert a tracked "Y" at offset 1 → text "xY", "Y" is an insertion.
    insert_text_tracked_at(&loro, &path, 1, "Y", &ins).expect("tracked insert");

    // Deleting the own insertion hard-deletes it.
    let a = tracked_grapheme_delete(&loro, &path, 1, 2, Some(&del)).expect("del ins");
    assert_eq!(a, DeleteAction::HardDelete);
    assert_eq!(
        loro_to_document(&loro).unwrap().sections[0].blocks[0].clone(),
        Block::Para(vec![Inline::Str("x".into())])
    );

    // Now strike the remaining "x", then deleting it again is a no-op skip.
    tracked_grapheme_delete(&loro, &path, 0, 1, Some(&del)).expect("strike x");
    let b = tracked_grapheme_delete(&loro, &path, 0, 1, Some(&del)).expect("skip");
    assert_eq!(b, DeleteAction::Skip);
}

#[test]
fn resolves_changes_across_multiple_blocks() {
    let mut doc = Document::new();
    doc.sections = vec![Section::with_layout_and_blocks(
        PageLayout::default(),
        vec![
            Block::Para(vec![tracked(RevisionKind::Deletion, "gone")]),
            Block::Para(vec![tracked(RevisionKind::Insertion, "stays")]),
        ],
    )];
    let loro = document_to_loro(&doc).expect("to loro");
    assert_eq!(accept_reject_all_revisions(&loro, true).expect("accept"), 2);
    let back = loro_to_document(&loro).expect("rebuild");
    // First paragraph emptied (deletion accepted), second keeps its text.
    assert_eq!(
        crate::content::toc::inline_plain_text(match &back.sections[0].blocks[1] {
            Block::Para(i) => i,
            _ => panic!(),
        }),
        "stays"
    );
    assert!(!back.has_tracked_changes());
}
