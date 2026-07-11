// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tracked selection deletion ([`tracked_delete_selection_at`], Review tab
//! 4a.2): with track changes on, deleting a selection strikes the text through
//! (a `MARK_REVISION` deletion) instead of removing it, hard-deletes the
//! author's own tracked insertions, skips already-struck text, and — crucially —
//! preserves the paragraph marks between selected blocks (no merge). With no
//! mark it falls back to the hard-deleting [`delete_selection_at`].

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::revision::{RevisionKind, RevisionMark};
use loki_doc_model::{BlockPath, tracked_delete_selection_at};

fn para(s: &str) -> Block {
    Block::Para(vec![Inline::Str(s.into())])
}

/// A run carrying a tracked-insertion mark by `author`.
fn insertion(author: &str, text: &str) -> Inline {
    Inline::StyledRun(StyledRun {
        style_id: None,
        direct_props: Some(Box::new(CharProps {
            revision: Some(RevisionMark::new(RevisionKind::Insertion).with_author(author)),
            ..CharProps::default()
        })),
        content: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    })
}

fn del_mark() -> RevisionMark {
    RevisionMark::new(RevisionKind::Deletion).with_author("Me")
}

/// The flattened text of an inline and its revision kind (if any).
fn run_of(i: &Inline) -> (String, Option<RevisionKind>) {
    fn text(i: &Inline) -> String {
        match i {
            Inline::Str(s) => s.clone(),
            Inline::StyledRun(r) => r.content.iter().map(text).collect(),
            _ => String::new(),
        }
    }
    let kind = match i {
        Inline::StyledRun(r) => r
            .direct_props
            .as_ref()
            .and_then(|p| p.revision.as_ref())
            .map(|m| m.kind),
        _ => None,
    };
    (text(i), kind)
}

/// The (text, revision-kind) runs of top-level block `b` after a rebuild.
fn runs(doc: &Document, b: usize) -> Vec<(String, Option<RevisionKind>)> {
    let inlines = match &doc.sections[0].blocks[b] {
        Block::Para(i) | Block::Plain(i) => i.clone(),
        Block::StyledPara(p) => p.inlines.clone(),
        _ => Vec::new(),
    };
    inlines.iter().map(run_of).collect()
}

#[test]
fn strikes_a_single_block_selection() {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![para("Hello world")];
    let loro = document_to_loro(&doc).unwrap();

    // Strike "lo wor" (bytes 3..9) — the text stays, marked deleted.
    let p = BlockPath::block(0);
    let mark = del_mark();
    let (path, byte) = tracked_delete_selection_at(&loro, (&p, 3), (&p, 9), Some(&mark)).unwrap();
    assert_eq!((path, byte), (BlockPath::block(0), 3));

    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(
        runs(&rebuilt, 0),
        vec![
            ("Hel".into(), None),
            ("lo wor".into(), Some(RevisionKind::Deletion)),
            ("ld".into(), None),
        ]
    );
}

#[test]
fn hard_deletes_own_tracked_insertion_in_range() {
    let mut doc = Document::new();
    // "keep" + tracked-insertion "ins" + "tail".
    doc.sections[0].blocks = vec![Block::Para(vec![
        Inline::Str("keep".into()),
        insertion("Me", "ins"),
        Inline::Str("tail".into()),
    ])];
    let loro = document_to_loro(&doc).unwrap();

    // Select exactly the inserted run (bytes 4..7): un-typed, not struck.
    let p = BlockPath::block(0);
    let mark = del_mark();
    tracked_delete_selection_at(&loro, (&p, 4), (&p, 7), Some(&mark)).unwrap();

    let rebuilt = loro_to_document(&loro).unwrap();
    // "ins" is gone; the survivors carry no revision.
    let text: String = runs(&rebuilt, 0).into_iter().map(|(t, _)| t).collect();
    assert_eq!(text, "keeptail");
    assert!(runs(&rebuilt, 0).iter().all(|(_, k)| k.is_none()));
}

#[test]
fn already_struck_text_is_left_alone() {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![para("abcdef")];
    let loro = document_to_loro(&doc).unwrap();
    let p = BlockPath::block(0);
    let mark = del_mark();

    // Strike "cd" (2..4), then strike an overlapping "bcde" (1..5). The already
    // struck "cd" must not double-apply; the fresh "b" and "e" join the deletion.
    tracked_delete_selection_at(&loro, (&p, 2), (&p, 4), Some(&mark)).unwrap();
    tracked_delete_selection_at(&loro, (&p, 1), (&p, 5), Some(&mark)).unwrap();

    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(
        runs(&rebuilt, 0),
        vec![
            ("a".into(), None),
            ("bcde".into(), Some(RevisionKind::Deletion)),
            ("f".into(), None),
        ]
    );
}

#[test]
fn multi_block_strike_preserves_paragraph_marks() {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![para("foo"), para("mid"), para("bar")];
    let loro = document_to_loro(&doc).unwrap();

    // Select from block 0 byte 1 to block 2 byte 2: strike "oo", "mid", "ba".
    let start = BlockPath::block(0);
    let end = BlockPath::block(2);
    let mark = del_mark();
    let (path, byte) =
        tracked_delete_selection_at(&loro, (&start, 1), (&end, 2), Some(&mark)).unwrap();
    assert_eq!((path, byte), (BlockPath::block(0), 1));

    let rebuilt = loro_to_document(&loro).unwrap();
    // Three blocks survive — no merge — with their slices struck.
    assert_eq!(rebuilt.sections[0].blocks.len(), 3);
    assert_eq!(
        runs(&rebuilt, 0),
        vec![
            ("f".into(), None),
            ("oo".into(), Some(RevisionKind::Deletion))
        ]
    );
    assert_eq!(
        runs(&rebuilt, 1),
        vec![("mid".into(), Some(RevisionKind::Deletion))]
    );
    assert_eq!(
        runs(&rebuilt, 2),
        vec![
            ("ba".into(), Some(RevisionKind::Deletion)),
            ("r".into(), None)
        ]
    );
}

#[test]
fn no_mark_hard_deletes_and_merges() {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![para("foo"), para("bar")];
    let loro = document_to_loro(&doc).unwrap();

    // Tracking off (None) → the classic hard delete + merge.
    let start = BlockPath::block(0);
    let end = BlockPath::block(1);
    tracked_delete_selection_at(&loro, (&start, 1), (&end, 2), None).unwrap();

    let rebuilt = loro_to_document(&loro).unwrap();
    assert_eq!(rebuilt.sections[0].blocks.len(), 1);
    assert_eq!(runs(&rebuilt, 0), vec![("fr".into(), None)]);
}
