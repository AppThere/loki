// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph-mark tracked deletion (Review tab 4a.2): a paragraph whose mark (¶)
//! carries a `Deletion` revision on its `direct_char_props` round-trips through
//! the CRDT, enables the Accept/Reject buttons, and — on accept — merges with the
//! next paragraph (reject clears the mark, keeping them split). Plus the editor's
//! `set_para_mark_deletion` recording mutation.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::revision::{RevisionKind, RevisionMark};
use loki_doc_model::{BlockPath, accept_reject_all_revisions, set_para_mark_deletion};

fn plain(text: &str) -> Block {
    Block::Para(vec![Inline::Str(text.into())])
}

/// A styled paragraph whose *mark* carries a `Deletion` revision.
fn mark_deleted(text: &str) -> Block {
    Block::StyledPara(StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: Some(Box::new(CharProps {
            revision: Some(RevisionMark::new(RevisionKind::Deletion).with_author("Ada")),
            ..CharProps::default()
        })),
        inlines: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    })
}

/// The plain text of top-level block `i`.
fn text(doc: &Document, i: usize) -> String {
    let inlines = match &doc.sections[0].blocks[i] {
        Block::Para(v) | Block::Plain(v) => v,
        Block::StyledPara(p) => &p.inlines,
        _ => return String::new(),
    };
    inlines
        .iter()
        .map(|x| match x {
            Inline::Str(s) => s.as_str(),
            _ => "",
        })
        .collect()
}

/// The paragraph-mark revision kind of top-level block `i`.
fn mark_kind(doc: &Document, i: usize) -> Option<RevisionKind> {
    match &doc.sections[0].blocks[i] {
        Block::StyledPara(p) => p
            .direct_char_props
            .as_ref()
            .and_then(|c| c.revision.as_ref())
            .map(|m| m.kind),
        _ => None,
    }
}

fn two_para_mark_deleted() -> Document {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![mark_deleted("first"), plain("second")];
    doc
}

#[test]
fn para_mark_revision_round_trips_through_the_crdt() {
    let doc = two_para_mark_deleted();
    assert!(doc.has_tracked_changes());
    let back = loro_to_document(&document_to_loro(&doc).unwrap()).unwrap();
    assert_eq!(mark_kind(&back, 0), Some(RevisionKind::Deletion));
    assert!(back.has_tracked_changes());
}

#[test]
fn accept_all_merges_the_marked_paragraph_with_the_next() {
    let loro = document_to_loro(&two_para_mark_deleted()).unwrap();
    let resolved = accept_reject_all_revisions(&loro, true).unwrap();
    assert_eq!(resolved, 1);
    let back = loro_to_document(&loro).unwrap();
    assert_eq!(back.sections[0].blocks.len(), 1);
    assert_eq!(text(&back, 0), "firstsecond");
    assert!(!back.has_tracked_changes());
}

#[test]
fn reject_all_clears_the_mark_and_keeps_paragraphs_split() {
    let loro = document_to_loro(&two_para_mark_deleted()).unwrap();
    let resolved = accept_reject_all_revisions(&loro, false).unwrap();
    assert_eq!(resolved, 1);
    let back = loro_to_document(&loro).unwrap();
    assert_eq!(back.sections[0].blocks.len(), 2);
    assert_eq!(text(&back, 0), "first");
    assert_eq!(text(&back, 1), "second");
    assert!(!back.has_tracked_changes());
}

#[test]
fn pure_accept_reject_transforms_handle_the_mark() {
    let mut doc = two_para_mark_deleted();
    doc.accept_all_revisions();
    assert_eq!(doc.sections[0].blocks.len(), 1);
    assert_eq!(text(&doc, 0), "firstsecond");

    let mut doc = two_para_mark_deleted();
    doc.reject_all_revisions();
    assert_eq!(doc.sections[0].blocks.len(), 2);
    assert!(!doc.has_tracked_changes());
}

#[test]
fn set_para_mark_deletion_records_and_upgrades_a_plain_para() {
    // Two plain paragraphs; record a ¶-deletion on the first (Backspace at the
    // start of the second would call this on block 0).
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![plain("first"), plain("second")];
    let loro = document_to_loro(&doc).unwrap();

    let mark = RevisionMark::new(RevisionKind::Deletion).with_author("Ada");
    assert!(set_para_mark_deletion(&loro, 0, &mark).unwrap()); // recorded

    let back = loro_to_document(&loro).unwrap();
    assert_eq!(mark_kind(&back, 0), Some(RevisionKind::Deletion));
    assert_eq!(text(&back, 0), "first"); // text untouched
    assert!(back.has_tracked_changes());
}

#[test]
fn set_para_mark_deletion_declines_a_heading() {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![
        Block::Heading(1, NodeAttr::default(), vec![Inline::Str("Title".into())]),
        plain("body"),
    ];
    let loro = document_to_loro(&doc).unwrap();
    let mark = RevisionMark::new(RevisionKind::Deletion).with_author("Ada");
    // A heading is not a paragraph mark — declined, so the caller hard-merges.
    assert!(!set_para_mark_deletion(&loro, 0, &mark).unwrap());
}

/// The marked paragraph being last (no successor) just clears on accept.
#[test]
fn accept_of_a_trailing_mark_clears_without_merging() {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![plain("first"), mark_deleted("second")];
    let loro = document_to_loro(&doc).unwrap();
    accept_reject_all_revisions(&loro, true).unwrap();
    let back = loro_to_document(&loro).unwrap();
    assert_eq!(back.sections[0].blocks.len(), 2); // nothing to merge into
    assert!(!back.has_tracked_changes());
}

/// Confirms the merged paragraph is addressable afterwards (offsets sane).
#[test]
fn merged_paragraph_is_editable() {
    let loro = document_to_loro(&two_para_mark_deleted()).unwrap();
    accept_reject_all_revisions(&loro, true).unwrap();
    // "firstsecond" is one block now; a delete at the join must be in-range.
    let p = BlockPath::block(0);
    loki_doc_model::delete_text_at(&loro, &p, 5, 1).unwrap();
    let back = loro_to_document(&loro).unwrap();
    assert_eq!(text(&back, 0), "firstecond");
}
