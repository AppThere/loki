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
