// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! A tracked-change (revision) mark survives a Loro CRDT round-trip, so tracked
//! insertions/deletions persist through an edit cycle (Review tab, 4a.2).

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::layout::section::Section;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::revision::{RevisionKind, RevisionMark};

fn tracked_run(kind: RevisionKind, author: &str, text: &str) -> Inline {
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

/// Finds the first run in `inlines` that carries a revision mark.
fn first_revision(inlines: &[Inline]) -> Option<RevisionMark> {
    inlines.iter().find_map(|i| match i {
        Inline::StyledRun(run) => run.direct_props.as_ref().and_then(|p| p.revision.clone()),
        _ => None,
    })
}

#[test]
fn revision_mark_round_trips_through_the_bridge() {
    let mut doc = Document::new();
    doc.sections = vec![Section::with_layout_and_blocks(
        PageLayout::default(),
        vec![Block::Para(vec![
            Inline::Str("Kept ".into()),
            tracked_run(RevisionKind::Insertion, "Ada", "inserted"),
        ])],
    )];

    let loro = document_to_loro(&doc).expect("to loro");
    let back = loro_to_document(&loro).expect("rebuild");

    let Block::Para(inlines) = &back.sections[0].blocks[0] else {
        panic!("expected a paragraph");
    };
    let rev = first_revision(inlines).expect("the inserted run keeps its revision mark");
    assert_eq!(rev.kind, RevisionKind::Insertion);
    assert_eq!(rev.author.as_deref(), Some("Ada"));
}

#[test]
fn tracked_insert_marks_only_its_range_and_does_not_leak() {
    use loki_doc_model::loro_mutation::BlockPath;
    use loki_doc_model::{insert_text_at, insert_text_tracked_at};

    // Start with one empty paragraph.
    let mut doc = Document::new();
    doc.sections = vec![Section::with_layout_and_blocks(
        PageLayout::default(),
        vec![Block::Para(vec![])],
    )];
    let loro = document_to_loro(&doc).expect("to loro");
    let path = BlockPath::block(0);

    // Type "new" as a tracked insertion, then type " old" plainly right after it
    // (track-changes turned off). The revision must cover exactly "new".
    let mark = RevisionMark::new(RevisionKind::Insertion).with_author("Ada");
    insert_text_tracked_at(&loro, &path, 0, "new", &mark).expect("tracked insert");
    insert_text_at(&loro, &path, 3, " old").expect("plain insert");

    let back = loro_to_document(&loro).expect("rebuild");
    let Block::Para(inlines) = &back.sections[0].blocks[0] else {
        panic!("expected a paragraph");
    };
    // The tracked run carries the mark; the plainly-typed run does not (the mark
    // is `expand: None`, so it did not bleed onto " old").
    let tracked: String = inlines
        .iter()
        .filter(|i| match i {
            Inline::StyledRun(run) => run
                .direct_props
                .as_ref()
                .is_some_and(|p| p.revision.is_some()),
            _ => false,
        })
        .map(text_of)
        .collect();
    assert_eq!(tracked, "new", "only the tracked text carries a revision");
    // The full paragraph text is intact.
    let all: String = inlines.iter().map(text_of).collect();
    assert_eq!(all, "new old");
}

fn text_of(inline: &Inline) -> String {
    match inline {
        Inline::Str(s) => s.clone(),
        Inline::StyledRun(run) => run.content.iter().map(text_of).collect(),
        _ => String::new(),
    }
}

#[test]
fn track_changes_setting_serde_is_backward_compatible() {
    use loki_doc_model::settings::DocumentSettings;

    // Enabled flag survives a serde round-trip.
    let on = DocumentSettings {
        track_changes: true,
        ..DocumentSettings::default()
    };
    let json = serde_json::to_string(&on).expect("serialize");
    let back: DocumentSettings = serde_json::from_str(&json).expect("deserialize");
    assert!(back.track_changes);

    // Old settings JSON without the field defaults to `false` (`#[serde(default)]`).
    let old: DocumentSettings =
        serde_json::from_str("{\"default_tab_stop_pt\":36.0}").expect("legacy deserialize");
    assert!(!old.track_changes);
}

#[test]
fn document_settings_round_trip_through_the_bridge() {
    use loki_doc_model::settings::DocumentSettings;

    let mut doc = Document::new();
    doc.settings = Some(DocumentSettings {
        track_changes: true,
        ..DocumentSettings::default()
    });
    let back = loro_to_document(&document_to_loro(&doc).expect("to loro")).expect("rebuild");
    assert_eq!(
        back.settings.map(|s| s.track_changes),
        Some(true),
        "settings now survive the CRDT round-trip",
    );
}
