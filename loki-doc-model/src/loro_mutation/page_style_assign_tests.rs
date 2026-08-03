// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for `create_page_style` / `set_section_page_style` — defining a page
//! style and putting it on a section.
//!
//! The load-bearing assertion in most of these is not that the reference moved
//! but that the **geometry** did: a reference the renderer does not follow is
//! the defect these two exist to avoid.

use loro::LoroDoc;

use super::{create_page_style, set_section_page_style};
use crate::content::block::Block;
use crate::content::inline::Inline;
use crate::document::Document;
use crate::layout::page::{PageLayout, PageOrientation, PageSize};
use crate::layout::section::Section;
use crate::loki_primitives::units::Points;
use crate::loro_bridge::{document_to_loro, loro_to_document};
use crate::style::catalog::StyleId;

/// A two-section A4 document; both sections share `PageStyle1`.
fn two_section_doc() -> LoroDoc {
    let section = || {
        Section::with_layout_and_blocks(
            PageLayout::default(),
            vec![Block::Para(vec![Inline::Str("x".into())])],
        )
    };
    let mut doc = Document::new();
    doc.sections = vec![section(), section()];
    doc.assign_page_styles();
    document_to_loro(&doc).expect("to loro")
}

/// A landscape Letter geometry, distinguishable from the default on every axis
/// the mutation writes.
fn landscape_letter() -> PageLayout {
    let base = PageSize::letter();
    PageLayout {
        page_size: PageSize {
            width: base.height,
            height: base.width,
        },
        orientation: PageOrientation::Landscape,
        margins: {
            let mut m = PageLayout::default().margins;
            m.left = Points::new(90.0);
            m
        },
        ..Default::default()
    }
}

#[test]
fn create_adds_a_catalogued_style_no_section_uses_yet() {
    let loro = two_section_doc();
    create_page_style(&loro, "Landscape", &landscape_letter()).expect("create");

    let doc = loro_to_document(&loro).expect("rebuild");
    let entry = doc
        .styles
        .page_styles
        .get(&StyleId::new("Landscape"))
        .expect("catalogued");
    assert_eq!(entry.layout.orientation, PageOrientation::Landscape);
    // No section references it, so nothing on screen changed.
    assert!(
        doc.sections
            .iter()
            .all(|s| s.page_style != Some(StyleId::new("Landscape")))
    );
    assert_eq!(
        doc.sections[0].layout.orientation,
        PageOrientation::Portrait
    );
}

#[test]
fn create_refuses_an_existing_name_and_an_empty_one() {
    let loro = two_section_doc();
    let before = loro_to_document(&loro).expect("rebuild");
    let count = before.styles.page_styles.len();

    // An existing name must not be overwritten with the new geometry.
    create_page_style(&loro, "PageStyle1", &landscape_letter()).expect("no-op");
    let doc = loro_to_document(&loro).expect("rebuild");
    assert_eq!(doc.styles.page_styles.len(), count);
    assert_eq!(
        doc.styles
            .page_styles
            .get(&StyleId::new("PageStyle1"))
            .map(|ps| ps.layout.orientation),
        Some(PageOrientation::Portrait),
        "an existing page style was clobbered"
    );

    create_page_style(&loro, "", &landscape_letter()).expect("no-op");
    assert_eq!(
        loro_to_document(&loro)
            .expect("rebuild")
            .styles
            .page_styles
            .len(),
        count
    );
}

/// The whole point of the pair: applying a created style moves the section's
/// geometry, not just its label.
#[test]
fn applying_a_created_style_gives_the_section_its_geometry() {
    let loro = two_section_doc();
    create_page_style(&loro, "Landscape", &landscape_letter()).expect("create");
    set_section_page_style(&loro, 1, "Landscape").expect("apply");

    let doc = loro_to_document(&loro).expect("rebuild");
    assert_eq!(doc.sections[1].page_style, Some(StyleId::new("Landscape")));
    let l = &doc.sections[1].layout;
    assert_eq!(l.orientation, PageOrientation::Landscape);
    assert!(
        l.page_size.width.value() > l.page_size.height.value(),
        "section kept portrait dimensions after applying a landscape page style"
    );
    assert_eq!(l.margins.left.value(), 90.0);

    // Section 0 is untouched — still portrait A4 under PageStyle1.
    assert_eq!(doc.sections[0].page_style, Some(StyleId::new("PageStyle1")));
    assert_eq!(
        doc.sections[0].layout.orientation,
        PageOrientation::Portrait
    );
    assert_eq!(doc.sections[0].layout.margins.left.value(), 72.0);
}

/// When the target style is already on another section, that live section's
/// geometry wins over the catalog copy — the renderer's own source.
///
/// The two agree whenever `set_page_style_geometry` made the change, so the
/// discriminating case has to come from a path that writes sections *without*
/// the catalog: the Layout ribbon's document-wide `set_document_*` mutations do
/// exactly that. Without the preference, applying a page style after a ribbon
/// margin change hands the new section the geometry the document had before it.
#[test]
fn a_live_section_outranks_a_catalog_entry_the_ribbon_left_behind() {
    let loro = two_section_doc();
    create_page_style(&loro, "Landscape", &landscape_letter()).expect("create");
    set_section_page_style(&loro, 0, "Landscape").expect("apply to 0");

    // The Layout ribbon: document-wide margins, sections only, catalog untouched.
    super::super::set_document_margins(&loro, 72.0, 72.0, 123.0, 72.0).expect("ribbon");
    let mid = loro_to_document(&loro).expect("rebuild");
    assert_eq!(
        mid.styles
            .page_styles
            .get(&StyleId::new("Landscape"))
            .map(|ps| ps.layout.margins.left.value()),
        Some(90.0),
        "fixture broken: the catalog entry was expected to be left stale here"
    );

    set_section_page_style(&loro, 1, "Landscape").expect("apply to 1");
    let doc = loro_to_document(&loro).expect("rebuild");
    assert_eq!(
        doc.sections[1].layout.margins.left.value(),
        123.0,
        "applied the stale catalog geometry instead of the live section's"
    );
}

#[test]
fn applying_an_unknown_style_or_an_out_of_range_section_is_a_no_op() {
    let loro = two_section_doc();
    let before = loro_to_document(&loro).expect("rebuild");

    set_section_page_style(&loro, 0, "Ghost").expect("no-op");
    let after = loro_to_document(&loro).expect("rebuild");
    assert_eq!(after.sections[0].page_style, before.sections[0].page_style);

    create_page_style(&loro, "Landscape", &landscape_letter()).expect("create");
    set_section_page_style(&loro, 99, "Landscape").expect("no-op");
    let after = loro_to_document(&loro).expect("rebuild");
    assert_eq!(after.sections.len(), before.sections.len());
    assert!(
        after
            .sections
            .iter()
            .all(|s| s.page_style != Some(StyleId::new("Landscape")))
    );
}
