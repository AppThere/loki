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
