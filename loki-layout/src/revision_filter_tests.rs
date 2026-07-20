// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use std::borrow::Cow;

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::revision::{RevisionKind, RevisionMark};

use super::display_inlines;
use crate::options::RevisionDisplay;

fn tracked_run(text: &str, kind: RevisionKind) -> Inline {
    Inline::StyledRun(StyledRun {
        style_id: None,
        direct_props: Some(Box::new(CharProps {
            revision: Some(RevisionMark {
                kind,
                author: Some("Ada".into()),
                date: None,
                id: Some("1".into()),
            }),
            ..Default::default()
        })),
        content: vec![Inline::Str(text.to_string())],
        attr: NodeAttr::default(),
    })
}

/// `[plain, ins, del]` under each display mode.
fn sample() -> Vec<Inline> {
    vec![
        Inline::Str("keep ".to_string()),
        tracked_run("added", RevisionKind::Insertion),
        tracked_run("removed", RevisionKind::Deletion),
    ]
}

fn run_revision(inl: &Inline) -> Option<RevisionKind> {
    match inl {
        Inline::StyledRun(r) => r
            .direct_props
            .as_deref()
            .and_then(|p| p.revision.as_ref())
            .map(|r| r.kind),
        _ => None,
    }
}

#[test]
fn all_markup_returns_the_input_borrowed_unchanged() {
    let inl = sample();
    let out = display_inlines(&inl, RevisionDisplay::AllMarkup);
    assert!(matches!(out, Cow::Borrowed(_)));
    assert_eq!(out.len(), 3);
}

#[test]
fn paragraph_without_revisions_is_borrowed_even_in_final() {
    let inl = vec![Inline::Str("plain".to_string())];
    let out = display_inlines(&inl, RevisionDisplay::Final);
    assert!(matches!(out, Cow::Borrowed(_)));
}

#[test]
fn final_hides_deletions_and_normalises_insertions() {
    let inl = sample();
    let out = display_inlines(&inl, RevisionDisplay::Final);
    // plain + the (now un-tracked) insertion; the deletion is gone.
    assert_eq!(out.len(), 2);
    assert!(matches!(&out[0], Inline::Str(s) if s == "keep "));
    // The insertion survives as normal text with no revision mark.
    assert!(matches!(&out[1], Inline::StyledRun(r)
        if matches!(&r.content[..], [Inline::Str(s)] if s == "added")));
    assert_eq!(run_revision(&out[1]), None, "insertion revision stripped");
}

#[test]
fn original_hides_insertions_and_normalises_deletions() {
    let inl = sample();
    let out = display_inlines(&inl, RevisionDisplay::Original);
    // plain + the (now un-tracked) deletion; the insertion is gone.
    assert_eq!(out.len(), 2);
    assert!(matches!(&out[1], Inline::StyledRun(r)
        if matches!(&r.content[..], [Inline::Str(s)] if s == "removed")));
    assert_eq!(run_revision(&out[1]), None, "deletion revision stripped");
}
