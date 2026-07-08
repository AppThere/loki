// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Per-change accept/reject at the caret ([`accept_reject_revision_at`] +
//! [`revision_at`], Review tab 4a.2): resolving the single tracked change under
//! the cursor (accept keeps an insertion / removes a deletion; reject inverts),
//! leaving the document's other changes untouched.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::revision::{RevisionKind, RevisionMark};
use loki_doc_model::{BlockPath, accept_reject_revision_at, revision_at};

/// A run carrying a tracked mark of `kind` by `author`.
fn tracked(kind: RevisionKind, author: &str, text: &str) -> Inline {
    Inline::StyledRun(StyledRun {
        style_id: None,
        direct_props: Some(Box::new(CharProps {
            revision: Some(RevisionMark::new(kind).with_author(author)),
            ..CharProps::default()
        })),
        content: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    })
}

/// `(text, revision-kind)` runs of top-level block 0 after a rebuild.
fn runs(doc: &Document) -> Vec<(String, Option<RevisionKind>)> {
    fn text(i: &Inline) -> String {
        match i {
            Inline::Str(s) => s.clone(),
            Inline::StyledRun(r) => r.content.iter().map(text).collect(),
            _ => String::new(),
        }
    }
    let inlines = match &doc.sections[0].blocks[0] {
        Block::Para(i) | Block::Plain(i) => i.clone(),
        Block::StyledPara(p) => p.inlines.clone(),
        _ => Vec::new(),
    };
    inlines
        .iter()
        .map(|i| {
            let kind = match i {
                Inline::StyledRun(r) => r
                    .direct_props
                    .as_ref()
                    .and_then(|p| p.revision.as_ref())
                    .map(|m| m.kind),
                _ => None,
            };
            (text(i), kind)
        })
        .collect()
}

/// A doc whose only block is "keep" + a tracked `kind` run "chg" + "tail"
/// (the tracked run occupies bytes 4..7).
fn doc_with_change(kind: RevisionKind) -> Document {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(vec![
        Inline::Str("keep".into()),
        tracked(kind, "Ada", "chg"),
        Inline::Str("tail".into()),
    ])];
    doc
}

#[test]
fn accept_insertion_clears_the_mark_and_keeps_text() {
    let loro = document_to_loro(&doc_with_change(RevisionKind::Insertion)).unwrap();
    let p = BlockPath::block(0);
    // Caret inside the inserted run (byte 5).
    let caret = accept_reject_revision_at(&loro, &p, 5, true).unwrap();
    assert_eq!(caret, Some(5)); // text kept, caret unchanged
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(runs(&rebuilt), vec![("keepchgtail".into(), None)]);
}

#[test]
fn reject_insertion_removes_the_text() {
    let loro = document_to_loro(&doc_with_change(RevisionKind::Insertion)).unwrap();
    let p = BlockPath::block(0);
    let caret = accept_reject_revision_at(&loro, &p, 5, false).unwrap();
    assert_eq!(caret, Some(4)); // collapses to the change start
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(runs(&rebuilt), vec![("keeptail".into(), None)]);
}

#[test]
fn accept_deletion_removes_the_text() {
    let loro = document_to_loro(&doc_with_change(RevisionKind::Deletion)).unwrap();
    let p = BlockPath::block(0);
    let caret = accept_reject_revision_at(&loro, &p, 5, true).unwrap();
    assert_eq!(caret, Some(4));
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(runs(&rebuilt), vec![("keeptail".into(), None)]);
}

#[test]
fn reject_deletion_restores_the_text() {
    let loro = document_to_loro(&doc_with_change(RevisionKind::Deletion)).unwrap();
    let p = BlockPath::block(0);
    let caret = accept_reject_revision_at(&loro, &p, 5, false).unwrap();
    assert_eq!(caret, Some(5));
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(runs(&rebuilt), vec![("keepchgtail".into(), None)]);
}

#[test]
fn no_change_at_caret_is_a_noop() {
    let loro = document_to_loro(&doc_with_change(RevisionKind::Insertion)).unwrap();
    let p = BlockPath::block(0);
    // Caret in the plain "keep" prefix (byte 1) — nothing to resolve.
    assert_eq!(accept_reject_revision_at(&loro, &p, 1, true).unwrap(), None);
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(
        runs(&rebuilt),
        vec![
            ("keep".into(), None),
            ("chg".into(), Some(RevisionKind::Insertion)),
            ("tail".into(), None),
        ]
    );
}

#[test]
fn revision_at_reports_the_caret_change() {
    let loro = document_to_loro(&doc_with_change(RevisionKind::Insertion)).unwrap();
    let p = BlockPath::block(0);
    assert!(revision_at(&loro, &p, 5)); // inside the change
    assert!(!revision_at(&loro, &p, 1)); // in plain text
}

#[test]
fn only_the_caret_change_is_resolved() {
    // "one"[ins by Ada] + "mid" + "two"[ins by Bob]: accept the first only.
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(vec![
        tracked(RevisionKind::Insertion, "Ada", "one"),
        Inline::Str("mid".into()),
        tracked(RevisionKind::Insertion, "Bob", "two"),
    ])];
    let loro = document_to_loro(&doc).unwrap();
    let p = BlockPath::block(0);
    // Caret inside "one" (byte 1) — accept only that change.
    accept_reject_revision_at(&loro, &p, 1, true).unwrap();
    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(
        runs(&rebuilt),
        vec![
            ("onemid".into(), None),
            ("two".into(), Some(RevisionKind::Insertion)),
        ]
    );
}
