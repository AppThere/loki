// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Hard line breaks survive the CRDT round trip, and soft breaks stay soft.
//!
//! The pure segmentation is unit-tested in `loro_bridge::line_breaks`; what is
//! asserted here is the **wiring** — that a `LineBreak` put into a document
//! comes back as a `LineBreak` after `document_to_loro → loro_to_document`.
//! That is the property Shift+Enter and DOCX `<w:br/>` export both depend on,
//! and the unit tests cannot see it: they exercise the helper, not the read
//! path that calls it.
//!
//! Before this, `LineBreak` was written as `\n` and reconstructed as an
//! `Inline::Str`, so a break was lost on the first re-derive — which happens on
//! every keystroke — and exported as `<w:t>` whitespace instead of `<w:br/>`.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};

fn round_trip(inlines: Vec<Inline>) -> Vec<Inline> {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(inlines)];
    let loro = document_to_loro(&doc).expect("document_to_loro must succeed");
    let back = loro_to_document(&loro).expect("loro_to_document must succeed");
    match back.sections[0].blocks.first() {
        Some(Block::Para(i)) => i.clone(),
        other => panic!("expected a paragraph, got {other:?}"),
    }
}

/// The headline property: a hard break comes back a hard break.
#[test]
fn a_line_break_survives_the_crdt_round_trip() {
    let back = round_trip(vec![
        Inline::Str("before".into()),
        Inline::LineBreak,
        Inline::Str("after".into()),
    ]);
    assert!(
        back.iter().any(|i| matches!(i, Inline::LineBreak)),
        "the line break did not survive: {back:?}"
    );
    // And the text is intact on both sides of it.
    let text: String = back
        .iter()
        .map(|i| match i {
            Inline::Str(s) => s.clone(),
            Inline::LineBreak => "\n".to_owned(),
            other => panic!("unexpected inline {other:?}"),
        })
        .collect();
    assert_eq!(text, "before\nafter");
}

/// The inversion. A paragraph with **no** break must not acquire one — a read
/// path that split on the wrong thing, or emitted a break per span, would pass
/// the test above and fail this one.
#[test]
fn ordinary_text_gains_no_line_break() {
    let back = round_trip(vec![Inline::Str("no breaks here".into())]);
    assert!(
        !back.iter().any(|i| matches!(i, Inline::LineBreak)),
        "a break appeared where none was written: {back:?}"
    );
}

/// A soft break is a *space* to every consumer — layout's `resolve_walk` and
/// the DOCX writer both render one — so it must not come back as a hard break.
///
/// The write path used to store `SoftBreak` and `LineBreak` as the same `\n`,
/// which meant that once the read path learned to reconstruct breaks, a soft
/// break would have been promoted to a hard one. Separating them on write is
/// what keeps this true.
#[test]
fn a_soft_break_does_not_become_a_hard_break() {
    let back = round_trip(vec![
        Inline::Str("a".into()),
        Inline::SoftBreak,
        Inline::Str("b".into()),
    ]);
    assert!(
        !back.iter().any(|i| matches!(i, Inline::LineBreak)),
        "a soft break was promoted to a hard break: {back:?}"
    );
    let text: String = back
        .iter()
        .map(|i| match i {
            Inline::Str(s) => s.clone(),
            other => panic!("unexpected inline {other:?}"),
        })
        .collect();
    assert_eq!(text, "a b", "a soft break should read back as a space");
}

/// Consecutive breaks keep their count — an empty paragraph line between two
/// runs is two breaks, not one.
#[test]
fn consecutive_line_breaks_keep_their_count() {
    let back = round_trip(vec![
        Inline::Str("a".into()),
        Inline::LineBreak,
        Inline::LineBreak,
        Inline::Str("b".into()),
    ]);
    assert_eq!(
        back.iter()
            .filter(|i| matches!(i, Inline::LineBreak))
            .count(),
        2,
        "expected two breaks: {back:?}"
    );
}
