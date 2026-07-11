// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for `set_page_style_geometry` — the per-page-style edit primitive.
//!
//! Unlike the document-wide `set_document_*` mutations, this applies a layout to
//! only the sections that belong to one page style (LibreOffice's model), so
//! editing one page style leaves the others untouched.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::{PageLayout, PageOrientation, PageSize, SectionColumns};
use loki_doc_model::layout::section::Section;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::style::{StyleId, section_page_style_ids};
use loki_doc_model::{rename_page_style, set_page_style_geometry};
use loro::LoroDoc;

/// A three-section document: A4, Letter, A4 — so there are two page styles
/// (PageStyle1 = A4 covering sections 0 & 2; PageStyle2 = Letter covering 1).
fn three_section_doc() -> LoroDoc {
    let section = |size: PageSize| {
        Section::with_layout_and_blocks(
            PageLayout {
                page_size: size,
                ..Default::default()
            },
            vec![Block::Para(vec![Inline::Str("x".into())])],
        )
    };
    let mut doc = Document::new();
    doc.sections = vec![
        section(PageSize::a4()),
        section(PageSize::letter()),
        section(PageSize::a4()),
    ];
    document_to_loro(&doc).expect("to loro")
}

/// The section indices belonging to page style `name`, derived from the sections.
fn indices_for(loro: &LoroDoc, name: &str) -> Vec<usize> {
    let doc = loro_to_document(loro).expect("rebuild");
    section_page_style_ids(&doc.sections)
        .iter()
        .enumerate()
        .filter(|(_, id)| id.as_str() == name)
        .map(|(i, _)| i)
        .collect()
}

#[test]
fn editing_a_page_style_changes_only_its_sections() {
    let loro = three_section_doc();
    // PageStyle1 covers the two A4 sections (0 and 2).
    let targets = indices_for(&loro, "PageStyle1");
    assert_eq!(targets, vec![0, 2]);

    // Make PageStyle1 landscape A4; leave PageStyle2 (Letter, section 1) alone.
    let landscape_a4 = PageLayout {
        page_size: PageSize {
            width: PageSize::a4().height,
            height: PageSize::a4().width,
        },
        orientation: PageOrientation::Landscape,
        ..Default::default()
    };
    set_page_style_geometry(&loro, &targets, &landscape_a4).expect("apply");

    let doc = loro_to_document(&loro).expect("rebuild");
    // Sections 0 and 2 are now landscape (width > height).
    for i in [0, 2] {
        let l = &doc.sections[i].layout;
        assert_eq!(l.orientation, PageOrientation::Landscape);
        assert!(l.page_size.width.value() > l.page_size.height.value());
    }
    // Section 1 (the Letter page style) is unchanged: still portrait Letter.
    let mid = &doc.sections[1].layout;
    assert_eq!(mid.orientation, PageOrientation::Portrait);
    assert!(mid.page_size.width.value() < mid.page_size.height.value());
    assert!((mid.page_size.width.value() - PageSize::letter().width.value()).abs() < 1.0);
}

#[test]
fn margins_and_columns_apply_to_the_targeted_sections() {
    let loro = three_section_doc();
    let targets = indices_for(&loro, "PageStyle2"); // the single Letter section (1)
    assert_eq!(targets, vec![1]);

    let mut layout = PageLayout {
        page_size: PageSize::letter(),
        ..Default::default()
    };
    layout.margins.left = loki_doc_model::loki_primitives::units::Points::new(144.0);
    layout.margins.right = loki_doc_model::loki_primitives::units::Points::new(144.0);
    layout.columns = Some(SectionColumns::two_column());
    set_page_style_geometry(&loro, &targets, &layout).expect("apply");

    let doc = loro_to_document(&loro).expect("rebuild");
    let sec = &doc.sections[1].layout;
    assert_eq!(sec.margins.left.value(), 144.0);
    assert_eq!(sec.columns.as_ref().map(|c| c.count), Some(2));
    // The A4 sections keep single-column default margins.
    assert_eq!(doc.sections[0].layout.margins.left.value(), 72.0);
}

#[test]
fn rename_updates_the_catalog_key_and_every_section_reference() {
    // Assign named page styles first (so sections carry refs + the catalog has
    // entries), then persist to the CRDT.
    let mut doc = loro_to_document(&three_section_doc()).expect("rebuild");
    doc.assign_page_styles();
    let loro = document_to_loro(&doc).expect("to loro");

    // PageStyle1 covers sections 0 and 2. Rename it to "Body".
    rename_page_style(&loro, "PageStyle1", "Body").expect("rename");

    let back = loro_to_document(&loro).expect("rebuild");
    assert!(back.styles.page_styles.contains_key(&StyleId::new("Body")));
    assert!(
        !back
            .styles
            .page_styles
            .contains_key(&StyleId::new("PageStyle1"))
    );
    assert_eq!(back.sections[0].page_style, Some(StyleId::new("Body")));
    assert_eq!(back.sections[2].page_style, Some(StyleId::new("Body")));
    // The Letter page style (section 1) is untouched.
    assert_eq!(
        back.sections[1].page_style,
        Some(StyleId::new("PageStyle2"))
    );
    // The renamed style keeps its own id in sync.
    assert_eq!(
        back.styles
            .page_styles
            .get(&StyleId::new("Body"))
            .unwrap()
            .id,
        StyleId::new("Body")
    );
}

#[test]
fn rename_is_a_no_op_on_conflict_or_missing() {
    let mut doc = loro_to_document(&three_section_doc()).expect("rebuild");
    doc.assign_page_styles();
    let loro = document_to_loro(&doc).expect("to loro");

    // Target name already exists → no merge.
    rename_page_style(&loro, "PageStyle1", "PageStyle2").expect("no-op");
    let back = loro_to_document(&loro).expect("rebuild");
    assert!(
        back.styles
            .page_styles
            .contains_key(&StyleId::new("PageStyle1"))
    );
    assert_eq!(back.styles.page_styles.len(), 2);

    // Unknown source → no-op.
    rename_page_style(&loro, "Ghost", "Whatever").expect("no-op");
    assert_eq!(loro_to_document(&loro).unwrap().styles.page_styles.len(), 2);
}

#[test]
fn out_of_range_indices_are_skipped() {
    let loro = three_section_doc();
    // Index 99 does not exist; the call must not error or touch anything.
    let before = loro_to_document(&loro).expect("rebuild");
    set_page_style_geometry(&loro, &[99], &PageLayout::default()).expect("no-op");
    let after = loro_to_document(&loro).expect("rebuild");
    assert_eq!(before.sections.len(), after.sections.len());
    assert_eq!(
        before.sections[0].layout.page_size.width.value(),
        after.sections[0].layout.page_size.width.value()
    );
}
