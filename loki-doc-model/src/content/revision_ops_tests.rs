// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for accept / reject of tracked changes.

use super::*;
use crate::content::attr::NodeAttr;
use crate::content::inline::StyledRun;
use crate::content::table::col::ColSpec;
use crate::content::table::core::{Table, TableBody, TableFoot, TableHead};
use crate::content::table::row::{Cell, Row};
use crate::layout::page::PageLayout;
use crate::layout::section::Section;
use crate::style::props::char_props::CharProps;
use crate::style::props::revision::{RevisionKind, RevisionMark};

/// A tracked run of `kind` carrying `text`.
fn tracked(kind: RevisionKind, text: &str) -> Inline {
    Inline::StyledRun(StyledRun {
        style_id: None,
        direct_props: Some(Box::new(CharProps {
            revision: Some(RevisionMark::new(kind)),
            ..CharProps::default()
        })),
        content: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    })
}

fn para(inlines: Vec<Inline>) -> Block {
    Block::Para(inlines)
}

/// Flattens the plain text of a paragraph's runs (order preserved).
fn para_text(block: &Block) -> String {
    let Block::Para(inlines) = block else {
        return String::new();
    };
    crate::content::toc::inline_plain_text(inlines)
}

/// "Keep " + inserted "new" + deleted "old".
fn mixed_para() -> Block {
    para(vec![
        Inline::Str("Keep ".into()),
        tracked(RevisionKind::Insertion, "new"),
        tracked(RevisionKind::Deletion, "old"),
    ])
}

#[test]
fn accept_keeps_insertions_and_drops_deletions() {
    let mut blocks = vec![mixed_para()];
    accept_revisions(&mut blocks);
    assert_eq!(para_text(&blocks[0]), "Keep new");
    // The surviving insertion run no longer carries a revision mark.
    assert!(!has_revisions(&blocks));
}

#[test]
fn reject_drops_insertions_and_keeps_deletions() {
    let mut blocks = vec![mixed_para()];
    reject_revisions(&mut blocks);
    assert_eq!(para_text(&blocks[0]), "Keep old");
    assert!(!has_revisions(&blocks));
}

#[test]
fn has_revisions_detects_and_clears() {
    let mut blocks = vec![mixed_para()];
    assert!(has_revisions(&blocks));
    accept_revisions(&mut blocks);
    assert!(!has_revisions(&blocks));
}

#[test]
fn resolves_inside_a_table_cell() {
    let cell = Cell::simple(vec![mixed_para()]);
    let table = Block::Table(Box::new(Table {
        attr: NodeAttr::default(),
        caption: Default::default(),
        width: None,
        col_specs: vec![ColSpec::proportional(1.0)],
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![Row::new(vec![cell])])],
        foot: TableFoot::empty(),
    }));
    let mut blocks = vec![table];
    assert!(has_revisions(&blocks));
    accept_revisions(&mut blocks);
    assert!(!has_revisions(&blocks));
    // The deletion inside the cell was removed; the insertion kept.
    let Block::Table(t) = &blocks[0] else {
        panic!("expected a table");
    };
    let cell_para = &t.bodies[0].body_rows[0].cells[0].blocks[0];
    assert_eq!(para_text(cell_para), "Keep new");
}

#[test]
fn document_level_accept_all_spans_sections() {
    let mut doc = Document::new();
    doc.sections = vec![
        Section::with_layout_and_blocks(PageLayout::default(), vec![mixed_para()]),
        Section::with_layout_and_blocks(PageLayout::default(), vec![mixed_para()]),
    ];
    assert!(doc.has_tracked_changes());
    doc.accept_all_revisions();
    assert!(!doc.has_tracked_changes());
    assert_eq!(para_text(&doc.sections[1].blocks[0]), "Keep new");
}

#[test]
fn untracked_content_is_untouched() {
    let mut blocks = vec![para(vec![Inline::Str("plain".into())])];
    let before = blocks.clone();
    accept_revisions(&mut blocks);
    assert_eq!(blocks, before);
}

#[test]
fn delete_action_matches_word_semantics() {
    // Tracking off ⇒ always a hard delete, whatever the grapheme carries.
    assert_eq!(delete_action(None, false), DeleteAction::HardDelete);
    assert_eq!(
        delete_action(Some(RevisionKind::Deletion), false),
        DeleteAction::HardDelete
    );
    // Tracking on: normal text is struck; own insertion is un-typed; an
    // already-struck deletion is skipped.
    assert_eq!(delete_action(None, true), DeleteAction::MarkDeleted);
    assert_eq!(
        delete_action(Some(RevisionKind::Insertion), true),
        DeleteAction::HardDelete
    );
    assert_eq!(
        delete_action(Some(RevisionKind::Deletion), true),
        DeleteAction::Skip
    );
}

#[test]
fn deletion_revision_follows_the_flag() {
    use crate::settings::DocumentSettings;
    let mut doc = Document::new();
    doc.meta.creator = Some("Ada".into());
    assert!(doc.deletion_revision().is_none());
    doc.settings = Some(DocumentSettings {
        track_changes: true,
        ..DocumentSettings::default()
    });
    let mark = doc.deletion_revision().expect("tracking on");
    assert_eq!(mark.kind, RevisionKind::Deletion);
    assert_eq!(mark.author.as_deref(), Some("Ada"));
}

#[test]
fn insertion_revision_follows_the_track_changes_flag() {
    use crate::settings::DocumentSettings;

    let mut doc = Document::new();
    doc.meta.creator = Some("Ada".into());

    // Off (no settings) → no mark.
    assert!(doc.insertion_revision().is_none());

    // On → an insertion attributed to the document author.
    doc.settings = Some(DocumentSettings {
        track_changes: true,
        ..DocumentSettings::default()
    });
    let mark = doc.insertion_revision().expect("tracking on ⇒ a mark");
    assert_eq!(mark.kind, RevisionKind::Insertion);
    assert_eq!(mark.author.as_deref(), Some("Ada"));

    // Explicitly off again → none.
    doc.settings = Some(DocumentSettings::default());
    assert!(doc.insertion_revision().is_none());
}
