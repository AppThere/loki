// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for `set_page_style_geometry` — the per-page-style edit primitive.
//!
//! Unlike the document-wide `set_document_*` mutations, this applies a layout to
//! only the sections that belong to one page style (LibreOffice's model), so
//! editing one page style leaves the others untouched.
//!
//! Every fixture here runs `assign_page_styles` before persisting, because the
//! **stored** `section.page_style` reference is what both the panel and the
//! mutation target. An earlier version of this file derived its indices from
//! `section_page_style_ids` (layout-equality grouping) instead, which agreed
//! with the stored refs on this fixture and would have disagreed the moment two
//! differently-named styles shared a geometry — it was reporting on the
//! neighbouring quantity.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::{PageLayout, PageOrientation, PageSize, SectionColumns};
use loki_doc_model::layout::section::Section;
use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::style::StyleId;
use loki_doc_model::{rename_page_style, set_page_style_geometry};
use loro::LoroDoc;

/// A three-section document: A4, Letter, A4 — so there are two page styles
/// (PageStyle1 = A4 covering sections 0 & 2; PageStyle2 = Letter covering 1),
/// with the stored references `assign_page_styles` gives every loaded document.
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
    doc.assign_page_styles();
    document_to_loro(&doc).expect("to loro")
}

#[test]
fn editing_a_page_style_changes_only_its_sections() {
    let loro = three_section_doc();

    // Make PageStyle1 (the two A4 sections, 0 and 2) landscape; leave PageStyle2
    // (Letter, section 1) alone.
    let landscape_a4 = PageLayout {
        page_size: PageSize {
            width: PageSize::a4().height,
            height: PageSize::a4().width,
        },
        orientation: PageOrientation::Landscape,
        ..Default::default()
    };
    set_page_style_geometry(&loro, "PageStyle1", &landscape_a4).expect("apply");

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

    let mut layout = PageLayout {
        page_size: PageSize::letter(),
        ..Default::default()
    };
    layout.margins.left = Points::new(144.0);
    layout.margins.right = Points::new(144.0);
    layout.columns = Some(SectionColumns::two_column());
    set_page_style_geometry(&loro, "PageStyle2", &layout).expect("apply");

    let doc = loro_to_document(&loro).expect("rebuild");
    let sec = &doc.sections[1].layout;
    assert_eq!(sec.margins.left.value(), 144.0);
    assert_eq!(sec.columns.as_ref().map(|c| c.count), Some(2));
    // The A4 sections keep single-column default margins.
    assert_eq!(doc.sections[0].layout.margins.left.value(), 72.0);
}

/// The catalog's copy of the geometry is written alongside the sections'. It is
/// what `set_section_page_style` seeds an unapplied style's first section from,
/// so a catalog entry left at its import-time layout would apply geometry no
/// page has shown.
#[test]
fn editing_geometry_updates_the_catalog_entry_too() {
    let loro = three_section_doc();
    let wide = PageLayout {
        margins: {
            let mut m = PageLayout::default().margins;
            m.left = Points::new(144.0);
            m
        },
        ..Default::default()
    };
    set_page_style_geometry(&loro, "PageStyle1", &wide).expect("apply");

    let doc = loro_to_document(&loro).expect("rebuild");
    let entry = doc
        .styles
        .page_styles
        .get(&StyleId::new("PageStyle1"))
        .expect("catalogued");
    assert_eq!(entry.layout.margins.left.value(), 144.0);
    // And the untouched style's entry still holds its own geometry.
    let other = doc
        .styles
        .page_styles
        .get(&StyleId::new("PageStyle2"))
        .expect("catalogued");
    assert_eq!(other.layout.margins.left.value(), 72.0);
}

/// Column widths are part of the geometry the mutation owns: a width list left
/// over from a different column count would be handed to the layout engine as
/// explicit widths the caller had already dropped.
#[test]
fn stale_column_widths_are_cleared_when_the_count_changes() {
    let loro = three_section_doc();

    let two_col = PageLayout {
        columns: Some(SectionColumns {
            count: 2,
            gap: Points::new(36.0),
            separator: false,
            widths: vec![Points::new(200.0), Points::new(235.0)],
        }),
        ..Default::default()
    };
    set_page_style_geometry(&loro, "PageStyle1", &two_col).expect("apply");
    let doc = loro_to_document(&loro).expect("rebuild");
    let cols = doc.sections[0].layout.columns.as_ref().expect("columns");
    assert_eq!(cols.widths.len(), 2, "explicit widths round-trip");

    // Now three equal columns: the two-entry width list must not survive.
    let three_col = PageLayout {
        columns: Some(SectionColumns {
            count: 3,
            gap: Points::new(36.0),
            separator: false,
            widths: Vec::new(),
        }),
        ..Default::default()
    };
    set_page_style_geometry(&loro, "PageStyle1", &three_col).expect("apply");
    let doc = loro_to_document(&loro).expect("rebuild");
    let cols = doc.sections[0].layout.columns.as_ref().expect("columns");
    assert_eq!(cols.count, 3);
    assert!(
        cols.widths.is_empty(),
        "stale 2-entry width list survived a change to 3 columns: {:?}",
        cols.widths
    );
}

#[test]
fn rename_updates_the_catalog_key_and_every_section_reference() {
    let loro = three_section_doc();

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
            .map(|ps| ps.id.clone()),
        Some(StyleId::new("Body"))
    );
}

#[test]
fn rename_is_a_no_op_on_conflict_or_missing() {
    let loro = three_section_doc();

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
    let back = loro_to_document(&loro).expect("rebuild");
    assert_eq!(back.styles.page_styles.len(), 2);
}

#[test]
fn an_unknown_page_style_name_touches_nothing() {
    let loro = three_section_doc();
    let before = loro_to_document(&loro).expect("rebuild");
    set_page_style_geometry(&loro, "Ghost", &PageLayout::default()).expect("no-op");
    let after = loro_to_document(&loro).expect("rebuild");
    assert_eq!(before.sections.len(), after.sections.len());
    for i in 0..before.sections.len() {
        assert_eq!(
            before.sections[i].layout.page_size.width.value(),
            after.sections[i].layout.page_size.width.value()
        );
    }
}
