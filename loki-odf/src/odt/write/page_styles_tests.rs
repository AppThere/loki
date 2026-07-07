// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the section → master-page name resolution.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::{PageLayout, PageSize};
use loki_doc_model::layout::section::Section;
use loki_doc_model::style::{PageStyle, StyleId};

use super::resolve_page_style_names;

fn section(size: PageSize, page_style: Option<&str>) -> Section {
    let mut s = Section::with_layout_and_blocks(
        PageLayout {
            page_size: size,
            ..Default::default()
        },
        vec![Block::Para(vec![Inline::Str("x".into())])],
    );
    s.page_style = page_style.map(StyleId::new);
    s
}

#[test]
fn stored_page_style_id_becomes_the_master_name() {
    let mut doc = Document::new();
    doc.sections = vec![
        section(PageSize::a4(), Some("Body")),
        section(PageSize::letter(), Some("Landscape")),
    ];
    let names = resolve_page_style_names(&doc);
    assert_eq!(names.section_master, vec!["Body", "Landscape"]);
    assert_eq!(names.masters.len(), 2);
    assert_eq!(names.masters[0].master, "Body");
    assert_eq!(names.masters[0].page_layout, "PL1");
    assert_eq!(names.masters[1].master, "Landscape");
    assert_eq!(names.masters[1].page_layout, "PL2");
}

#[test]
fn sections_sharing_a_page_style_share_one_master() {
    let mut doc = Document::new();
    doc.sections = vec![
        section(PageSize::a4(), Some("Body")),
        section(PageSize::letter(), Some("Body")),
        section(PageSize::a4(), Some("Cover")),
    ];
    let names = resolve_page_style_names(&doc);
    // Both "Body" sections reference the single "Body" master; only two distinct.
    assert_eq!(names.section_master, vec!["Body", "Body", "Cover"]);
    assert_eq!(names.masters.len(), 2);
    // The shared master keeps the FIRST referencing section's geometry (A4).
    let body = &names.masters[0];
    assert_eq!(body.master, "Body");
    assert!(body.layout.page_size.width.value() < body.layout.page_size.height.value());
}

#[test]
fn sections_without_a_page_style_fall_back_to_positional_names() {
    let mut doc = Document::new();
    doc.sections = vec![
        section(PageSize::a4(), None),
        section(PageSize::letter(), None),
    ];
    let names = resolve_page_style_names(&doc);
    // Positional fallback matches the pre-page-style behaviour exactly.
    assert_eq!(names.section_master, vec!["Standard", "MP1"]);
}

#[test]
fn ids_are_sanitised_to_valid_ncnames() {
    let mut doc = Document::new();
    doc.sections = vec![
        section(PageSize::a4(), Some("My Page")),
        section(PageSize::letter(), Some("2ndStyle")),
    ];
    let names = resolve_page_style_names(&doc);
    assert_eq!(names.section_master, vec!["My_Page", "_2ndStyle"]);
}

#[test]
fn a_distinct_catalog_display_name_is_carried_but_a_redundant_one_is_not() {
    let mut doc = Document::new();
    doc.sections = vec![
        section(PageSize::a4(), Some("WideBody")),
        section(PageSize::letter(), Some("Cover")),
    ];
    // "WideBody" has a human name distinct from its id; "Cover" repeats its id.
    let mut wide = PageStyle::new(StyleId::new("WideBody"), doc.sections[0].layout.clone());
    wide.display_name = Some("Wide Body".to_string());
    doc.styles
        .page_styles
        .insert(StyleId::new("WideBody"), wide);
    let mut cover = PageStyle::new(StyleId::new("Cover"), doc.sections[1].layout.clone());
    cover.display_name = Some("Cover".to_string());
    doc.styles.page_styles.insert(StyleId::new("Cover"), cover);

    let names = resolve_page_style_names(&doc);
    assert_eq!(names.masters[0].display_name.as_deref(), Some("Wide Body"));
    // A display name equal to the emitted name is redundant — left unset.
    assert_eq!(names.masters[1].display_name, None);
}

#[test]
fn an_empty_document_yields_one_default_master() {
    let doc = Document::new();
    let names = resolve_page_style_names(&doc);
    assert_eq!(names.masters.len(), 1);
    assert_eq!(names.section_master, vec!["Standard"]);
}
