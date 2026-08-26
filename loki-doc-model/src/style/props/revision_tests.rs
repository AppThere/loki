// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for revision-mark packing.

use super::*;

#[test]
fn round_trips_a_full_mark() {
    let mark = RevisionMark {
        kind: RevisionKind::Deletion,
        author: Some("Ada Lovelace".into()),
        date: Some("2026-07-07T12:00:00Z".into()),
        id: Some("rev-3".into()),
    };
    assert_eq!(decode(&encode(&mark)), Some(mark));
}

#[test]
fn round_trips_a_bare_mark() {
    let mark = RevisionMark::new(RevisionKind::Insertion);
    let decoded = decode(&encode(&mark)).expect("decodes");
    assert_eq!(decoded.kind, RevisionKind::Insertion);
    assert!(decoded.author.is_none() && decoded.date.is_none() && decoded.id.is_none());
}

#[test]
fn author_with_spaces_survives_verbatim() {
    let mark = RevisionMark::new(RevisionKind::Insertion).with_author("De  Morgan, A.");
    assert_eq!(
        decode(&encode(&mark)).and_then(|m| m.author),
        Some("De  Morgan, A.".to_string())
    );
}

#[test]
fn an_unknown_tag_is_rejected() {
    assert_eq!(decode("x\u{1f}\u{1f}\u{1f}"), None);
    assert_eq!(decode(""), None);
}

// ── Wire-format goldens (audit finding F7) ────────────────────────────────────
//
// The tests above are symmetric — `decode(&encode(..))` — so both halves come
// from the module under test and a coordinated change to the packing (a
// different separator, a reordered field list, a different kind tag) passes all
// of them. This encoding is **persisted**: it is the value of the `revision`
// mark inside Loro CRDT snapshots on disk and inside the collaboration server's
// oplogs, so changing it silently drops the tracked changes of every existing
// document.
//
// The two tests below pin the exact octets in both directions, with the strings
// written as literals. **Changing them is a breaking format change** requiring a
// migration (a decoder that still accepts the old shape, as
// `loro_bridge::decode_tests::v1_border_strings_still_decode` does for borders),
// not an updated expectation.

#[test]
fn encode_produces_the_pinned_wire_string() {
    let full = RevisionMark {
        kind: RevisionKind::Deletion,
        author: Some("Ada Lovelace".into()),
        date: Some("2026-07-07T12:00:00Z".into()),
        id: Some("rev-3".into()),
    };
    assert_eq!(
        encode(&full),
        "d\u{1f}Ada Lovelace\u{1f}2026-07-07T12:00:00Z\u{1f}rev-3"
    );

    // The insertion tag, and absent fields as empty (not omitted) slots — the
    // trailing separators must survive, or a decoder counting fields by
    // position mis-reads every bare mark.
    assert_eq!(
        encode(&RevisionMark::new(RevisionKind::Insertion)),
        "i\u{1f}\u{1f}\u{1f}"
    );
}

#[test]
fn decode_reads_the_pinned_wire_string() {
    let decoded = decode("d\u{1f}Ada Lovelace\u{1f}2026-07-07T12:00:00Z\u{1f}rev-3")
        .expect("the pinned wire string must decode");
    assert_eq!(decoded.kind, RevisionKind::Deletion);
    assert_eq!(decoded.author.as_deref(), Some("Ada Lovelace"));
    assert_eq!(decoded.date.as_deref(), Some("2026-07-07T12:00:00Z"));
    assert_eq!(decoded.id.as_deref(), Some("rev-3"));

    let bare = decode("i\u{1f}\u{1f}\u{1f}").expect("the bare wire string must decode");
    assert_eq!(bare, RevisionMark::new(RevisionKind::Insertion));

    // Inversion: the field order is load-bearing. The same three values in a
    // different order must NOT produce the mark above — otherwise the test
    // would pass against a decoder that ignores position.
    let scrambled = decode("d\u{1f}rev-3\u{1f}Ada Lovelace\u{1f}2026-07-07T12:00:00Z")
        .expect("still decodes — only the field assignment differs");
    assert_ne!(scrambled, decoded);
    assert_eq!(scrambled.author.as_deref(), Some("rev-3"));
}
