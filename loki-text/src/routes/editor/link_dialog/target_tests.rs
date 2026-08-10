// SPDX-License-Identifier: Apache-2.0

//! Tests for the in-document target outline.

use super::*;
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::layout::section::Section;

fn text(s: &str) -> Vec<Inline> {
    vec![Inline::Str(s.to_string())]
}

fn heading(level: u8, id: Option<&str>, label: &str) -> Block {
    let mut attr = NodeAttr::default();
    attr.id = id.map(str::to_string);
    Block::Heading(level, attr, text(label))
}

fn doc(blocks: Vec<Block>) -> Document {
    let mut d = Document::default();
    d.sections = vec![Section {
        blocks,
        ..Default::default()
    }];
    d
}

#[test]
fn headings_and_bookmarks_are_listed_in_document_order() {
    let targets = document_targets(&doc(vec![
        heading(1, Some("one"), "One · The harbour"),
        Block::Para(vec![Inline::Bookmark(
            BookmarkKind::Start,
            "tide-note".to_string(),
        )]),
        heading(2, Some("tide"), "The tide table"),
    ]));

    let labels: Vec<&str> = targets.iter().map(|t| t.label.as_str()).collect();
    assert_eq!(
        labels,
        vec!["One · The harbour", "tide-note", "The tide table"]
    );
    assert_eq!(targets[0].kind, TargetKind::Heading(1));
    assert_eq!(targets[1].kind, TargetKind::Bookmark);
    assert_eq!(targets[2].kind, TargetKind::Heading(2));
}

/// A heading with an id keeps it — the anchor must match what the exporter
/// writes, or every link breaks on export.
#[test]
fn an_existing_heading_id_is_used_as_the_anchor() {
    let targets = document_targets(&doc(vec![heading(1, Some("chapter-one"), "One")]));
    assert_eq!(targets[0].anchor, "chapter-one");
    assert_eq!(targets[0].href(), "#chapter-one");
}

/// Two headings with the same text must not share an anchor, or the second
/// link jumps to the first.
#[test]
fn duplicate_heading_text_produces_distinct_anchors() {
    let targets = document_targets(&doc(vec![
        heading(1, None, "Introduction"),
        heading(1, None, "Introduction"),
    ]));
    assert_eq!(targets.len(), 2);
    assert_ne!(targets[0].anchor, targets[1].anchor);
}

/// A blank heading cannot be told from its neighbours in a list of labels.
#[test]
fn an_empty_heading_is_not_offered_as_a_target() {
    let targets = document_targets(&doc(vec![
        heading(1, Some("blank"), "   "),
        heading(1, Some("real"), "Real"),
    ]));
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].label, "Real");
}

/// A range bookmark emits Start and End with the same name; offering both would
/// put two identical rows in the list pointing at opposite ends of one span.
#[test]
fn only_the_start_of_a_range_bookmark_is_a_target() {
    let targets = document_targets(&doc(vec![Block::Para(vec![
        Inline::Bookmark(BookmarkKind::Start, "range".to_string()),
        Inline::Str("body".to_string()),
        Inline::Bookmark(BookmarkKind::End, "range".to_string()),
    ])]));
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].kind, TargetKind::Bookmark);
}

/// Targets nested inside quotes, lists and table cells are still reachable —
/// a bookmark in a table cell is exactly the kind a cross-reference wants.
#[test]
fn nested_blocks_are_walked() {
    let targets = document_targets(&doc(vec![
        Block::BlockQuote(vec![heading(2, Some("quoted"), "Quoted")]),
        Block::BulletList(vec![vec![Block::Para(vec![Inline::Bookmark(
            BookmarkKind::Start,
            "in-list".to_string(),
        )])]]),
    ]));
    let anchors: Vec<&str> = targets.iter().map(|t| t.anchor.as_str()).collect();
    assert_eq!(anchors, vec!["quoted", "in-list"]);
}

/// A document with no headings or bookmarks yields an empty list, so the
/// picker can show its "nothing to link to" state rather than a blank box.
#[test]
fn a_document_with_no_anchors_yields_nothing() {
    assert!(document_targets(&doc(vec![Block::Para(text("body"))])).is_empty());
}

/// Depth drives the list's indent: headings nest by level, bookmarks never do.
#[test]
fn depth_follows_heading_level_and_bookmarks_stay_flat() {
    let targets = document_targets(&doc(vec![
        heading(1, Some("a"), "A"),
        heading(3, Some("b"), "B"),
        Block::Para(vec![Inline::Bookmark(
            BookmarkKind::Start,
            "mark".to_string(),
        )]),
    ]));
    assert_eq!(targets[0].depth(), 0);
    assert_eq!(targets[1].depth(), 2);
    assert_eq!(targets[2].depth(), 0);
}

/// An out-of-range level from an imported document must not overflow the
/// indent — it is clamped into the 1–6 outline the model documents.
#[test]
fn an_out_of_range_heading_level_is_clamped() {
    let targets = document_targets(&doc(vec![
        heading(0, Some("zero"), "Zero"),
        heading(9, Some("nine"), "Nine"),
    ]));
    assert_eq!(targets[0].kind, TargetKind::Heading(1));
    assert_eq!(targets[1].kind, TargetKind::Heading(6));
    assert!(targets[1].depth() <= 5);
}
