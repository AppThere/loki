// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use loki_doc_model::Document;
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{BookmarkKind, Inline, StyledRun};
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::props::char_props::CharProps;

use crate::roundtrip::diff_models;

/// A one-section document with the given blocks.
fn doc(blocks: Vec<Block>) -> Document {
    let mut d = Document::default();
    let mut s = Section::new();
    s.blocks = blocks;
    d.sections = vec![s];
    d
}

fn str_(s: &str) -> Inline {
    Inline::Str(s.to_string())
}

/// A run with the given direct character props around one text inline.
fn run(text: &str, props: Option<CharProps>) -> Inline {
    Inline::StyledRun(StyledRun {
        style_id: None,
        direct_props: props.map(Box::new),
        content: vec![str_(text)],
        attr: NodeAttr::default(),
    })
}

#[test]
fn identical_documents_round_trip_clean() {
    let d = doc(vec![Block::Para(vec![
        str_("hello"),
        Inline::Space,
        str_("world"),
    ])]);
    assert_eq!(diff_models(&d, &d.clone()), None);
}

#[test]
fn dropped_run_property_is_caught_with_a_path() {
    let bold = CharProps {
        bold: Some(true),
        ..Default::default()
    };
    let a = doc(vec![Block::Para(vec![run("x", Some(bold))])]);
    // Export drops the run's direct formatting entirely.
    let b = doc(vec![Block::Para(vec![run("x", None)])]);

    let d = diff_models(&a, &b).expect("dropped bold must be caught");
    assert!(d.path.ends_with("/props"), "path = {}", d.path);
    assert!(d.left.as_deref().unwrap_or_default().contains("bold=true"));
    assert!(d.right.is_none(), "right should lack the props entry");
}

#[test]
fn changed_run_property_reports_both_sides() {
    let a = doc(vec![Block::Para(vec![run(
        "x",
        Some(CharProps {
            bold: Some(true),
            ..Default::default()
        }),
    )])]);
    let b = doc(vec![Block::Para(vec![run(
        "x",
        Some(CharProps {
            italic: Some(true),
            ..Default::default()
        }),
    )])]);

    let d = diff_models(&a, &b).expect("bold→italic must be caught");
    assert!(d.path.ends_with("/props"));
    assert_eq!(d.left.as_deref(), Some("bold=true"));
    assert_eq!(d.right.as_deref(), Some("italic=true"));
}

#[test]
fn mangled_bookmark_id_is_caught() {
    let a = doc(vec![Block::Para(vec![
        Inline::Bookmark(BookmarkKind::Start, "_Ref1".to_string()),
        str_("anchored"),
    ])]);
    let b = doc(vec![Block::Para(vec![
        Inline::Bookmark(BookmarkKind::Start, "_Ref2".to_string()),
        str_("anchored"),
    ])]);

    let d = diff_models(&a, &b).expect("mangled bookmark id must be caught");
    assert!(d.path.ends_with("/id"), "path = {}", d.path);
    assert!(d.left.as_deref().unwrap_or_default().contains("_Ref1"));
    assert!(d.right.as_deref().unwrap_or_default().contains("_Ref2"));
}

#[test]
fn dropped_text_is_caught() {
    let a = doc(vec![Block::Para(vec![
        str_("hello"),
        Inline::Space,
        str_("world"),
    ])]);
    let b = doc(vec![Block::Para(vec![
        str_("hello"),
        Inline::Space,
        str_("there"),
    ])]);
    let d = diff_models(&a, &b).expect("changed word must be caught");
    assert!(d.path.ends_with("/str"));
    assert_eq!(d.left.as_deref(), Some("world"));
    assert_eq!(d.right.as_deref(), Some("there"));
}

#[test]
fn structural_change_is_caught_by_kind() {
    // A paragraph replaced by a heading: the `kind` entry diverges.
    let a = doc(vec![Block::Para(vec![str_("title")])]);
    let b = doc(vec![Block::Heading(
        1,
        NodeAttr::default(),
        vec![str_("title")],
    )]);
    let d = diff_models(&a, &b).expect("para→heading must be caught");
    assert!(d.path.ends_with("/kind"));
    assert_eq!(d.left.as_deref(), Some("para"));
    assert_eq!(d.right.as_deref(), Some("heading"));
}
