// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for named page styles: `Document::assign_page_styles` normalization and
//! the `Section.page_style` reference round-tripping through the Loro bridge.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::{PageLayout, PageSize};
use loki_doc_model::layout::section::Section;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::style::StyleId;

fn section(size: PageSize) -> Section {
    Section::with_layout_and_blocks(
        PageLayout {
            page_size: size,
            ..Default::default()
        },
        vec![Block::Para(vec![Inline::Str("x".into())])],
    )
}

fn three_section_doc() -> Document {
    let mut doc = Document::new();
    doc.sections = vec![
        section(PageSize::a4()),
        section(PageSize::letter()),
        section(PageSize::a4()),
    ];
    doc
}

#[test]
fn assign_dedups_and_references_shared_layouts() {
    let mut doc = three_section_doc();
    doc.assign_page_styles();

    // Two distinct page styles; the A4 sections share PageStyle1.
    assert_eq!(doc.styles.page_styles.len(), 2);
    assert_eq!(doc.sections[0].page_style, Some(StyleId::new("PageStyle1")));
    assert_eq!(doc.sections[1].page_style, Some(StyleId::new("PageStyle2")));
    assert_eq!(doc.sections[2].page_style, Some(StyleId::new("PageStyle1")));
    assert_eq!(
        doc.styles
            .page_styles
            .get(&StyleId::new("PageStyle1"))
            .unwrap()
            .layout
            .page_size,
        PageSize::a4()
    );
}

#[test]
fn assign_is_idempotent_and_preserves_existing_names() {
    let mut doc = three_section_doc();
    doc.assign_page_styles();
    // Simulate a user rename: section 1 references a custom-named style.
    doc.sections[1].page_style = Some(StyleId::new("Cover"));

    doc.assign_page_styles(); // re-run
    // The custom reference is preserved (only unassigned sections get styles).
    assert_eq!(doc.sections[1].page_style, Some(StyleId::new("Cover")));
    // The already-assigned A4 sections keep their style — no duplicates created.
    assert_eq!(doc.sections[0].page_style, Some(StyleId::new("PageStyle1")));
}

#[test]
fn page_style_reference_round_trips_through_the_bridge() {
    let mut doc = three_section_doc();
    doc.assign_page_styles();

    let loro = document_to_loro(&doc).expect("to loro");
    let back = loro_to_document(&loro).expect("rebuild");

    assert_eq!(
        back.sections[0].page_style,
        Some(StyleId::new("PageStyle1"))
    );
    assert_eq!(
        back.sections[1].page_style,
        Some(StyleId::new("PageStyle2"))
    );
    assert_eq!(
        back.sections[2].page_style,
        Some(StyleId::new("PageStyle1"))
    );
    // The page-styles catalog survives too (serialized with the catalog JSON).
    assert_eq!(back.styles.page_styles.len(), 2);
}

#[test]
fn a_section_without_a_named_style_round_trips_as_none() {
    // A fresh document's section has no page style until assigned.
    let doc = Document::new();
    assert_eq!(doc.sections[0].page_style, None);
    let back = loro_to_document(&document_to_loro(&doc).expect("to loro")).expect("rebuild");
    assert_eq!(back.sections[0].page_style, None);
}
