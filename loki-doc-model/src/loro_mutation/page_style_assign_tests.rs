// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for `create_page_style` / `set_section_page_style` — defining a page
//! style and putting it on a section.
//!
//! The load-bearing assertion in most of these is not that the reference moved
//! but that the **geometry** did: a reference the renderer does not follow is
//! the defect these two exist to avoid.

use loro::LoroDoc;

use super::{create_page_style, delete_page_style, set_section_page_style};
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

/// **Dropping to one column must clear the separator from the CRDT.**
///
/// The count key is rewritten every pass, but `gap` and `separator` were only
/// written when the new layout *had* columns — so going three-to-one left
/// `separator: true` behind, the reader rebuilt
/// `Some(SectionColumns { count: 1, separator: true })`, and ODT export wrote a
/// `<style:column-sep>` inside a one-column `<style:columns>`. The widths key
/// two lines below already had this rule; the other two did not.
#[test]
fn dropping_to_one_column_clears_the_separator_it_left_behind() {
    use crate::layout::page::SectionColumns;
    use crate::loro_mutation::set_page_style_geometry;

    let mut doc = Document::new();
    let mut section = Section::with_layout_and_blocks(
        PageLayout {
            columns: Some(SectionColumns {
                count: 3,
                gap: Points::new(18.0),
                separator: true,
                widths: Vec::new(),
            }),
            ..PageLayout::default()
        },
        vec![Block::Para(vec![Inline::Str("x".into())])],
    );
    section.page_style = Some(StyleId::new("Body"));
    doc.sections = vec![section];
    doc.assign_page_styles();
    doc.sections[0].page_style = Some(StyleId::new("Body"));

    let loro = document_to_loro(&doc).expect("seed");

    // The separator really is stored first, so the assertion below is not
    // passing on a document that never had one.
    let seeded = loro_to_document(&loro).expect("read back");
    assert!(
        seeded.sections[0]
            .layout
            .columns
            .as_ref()
            .is_some_and(|c| c.separator && c.count == 3),
        "fixture never stored a three-column separator"
    );

    let single = PageLayout {
        columns: None,
        ..PageLayout::default()
    };
    set_page_style_geometry(&loro, "Body", &single).expect("drop to one column");

    let after = loro_to_document(&loro).expect("read back");
    let cols = after.sections[0].layout.columns.as_ref();
    assert!(
        cols.is_none_or(|c| !c.separator),
        "a one-column layout came back carrying a separator: {cols:?}"
    );
}

// ── delete_page_style ─────────────────────────────────────────────────────────

/// **Deleting a page style removes the name and leaves the pages alone.**
///
/// The catalog entry is a name for a shape, not the shape. A delete that also
/// dropped the geometry would resize the user's pages when they asked only to
/// stop calling them "PageStyle1".
#[test]
fn delete_removes_the_name_and_keeps_the_geometry() {
    use crate::loro_mutation::set_page_style_geometry;

    let loro = two_section_doc();
    let landscape = landscape_letter();
    set_page_style_geometry(&loro, "PageStyle1", &landscape).expect("set geometry");

    // Establish the phenomenon before removing it: both sections must actually
    // carry the reference *and* the geometry, or the assertions below pass on a
    // document that never had either.
    let before = loro_to_document(&loro).expect("read back");
    assert!(
        before
            .sections
            .iter()
            .all(|s| s.page_style.as_ref().map(StyleId::as_str) == Some("PageStyle1")),
        "fixture sections do not reference the style"
    );
    assert!(
        before
            .styles
            .page_styles
            .contains_key(&StyleId::new("PageStyle1")),
        "fixture style is not in the catalog"
    );
    let geometry_before: Vec<_> = before.sections.iter().map(|s| s.layout.clone()).collect();

    delete_page_style(&loro, "PageStyle1").expect("delete");

    let after = loro_to_document(&loro).expect("read back");
    assert!(
        !after
            .styles
            .page_styles
            .contains_key(&StyleId::new("PageStyle1")),
        "the catalog entry survived the delete"
    );
    assert!(
        after.sections.iter().all(|s| s.page_style.is_none()),
        "a section still references the deleted style"
    );
    let geometry_after: Vec<_> = after.sections.iter().map(|s| s.layout.clone()).collect();
    assert_eq!(
        geometry_before, geometry_after,
        "deleting the name moved the pages"
    );
}

/// **The references must go too, or the style comes straight back.**
///
/// `panel_page_styles` lists referenced-but-uncatalogued styles as well — it has
/// to, or a catalog an importer left incomplete would orphan the only handle on
/// those sections. So dropping the catalog entry alone deletes nothing the user
/// can see: the name reappears in the list, sourced from the sections.
///
/// This is the assertion that fails if `delete_page_style` stops clearing the
/// references, which `delete_removes_the_name_and_keeps_the_geometry` also
/// covers — but this one states *why* in the terms the panel sees.
#[test]
fn a_deleted_style_is_not_reachable_through_a_section_reference() {
    let loro = two_section_doc();
    delete_page_style(&loro, "PageStyle1").expect("delete");
    let after = loro_to_document(&loro).expect("read back");

    let names: Vec<String> = after
        .sections
        .iter()
        .filter_map(|s| s.page_style.as_ref().map(|i| i.as_str().to_string()))
        .chain(
            after
                .styles
                .page_styles
                .keys()
                .map(|k| k.as_str().to_string()),
        )
        .collect();
    assert!(
        !names.iter().any(|n| n == "PageStyle1"),
        "the deleted style is still reachable: {names:?}"
    );
}

/// **A style referenced by a section but absent from the catalog is deletable.**
///
/// That combination is not hypothetical — the panel lists it, so the user can
/// select it, so delete must reach it. A guard written as "is it in the catalog"
/// would refuse, leaving a selectable style with a dead delete button.
#[test]
fn delete_reaches_a_style_the_catalog_never_had() {
    let loro = two_section_doc();
    // Point a section at a name the catalog does not contain.
    let sections = loro.get_list(crate::loro_schema::KEY_SECTIONS);
    let section = sections
        .get(0)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .expect("section 0");
    section
        .insert(crate::loro_schema::KEY_PAGE_STYLE_REF, "Uncatalogued")
        .expect("point at an uncatalogued name");

    let before = loro_to_document(&loro).expect("read back");
    assert_eq!(
        before.sections[0].page_style.as_ref().map(StyleId::as_str),
        Some("Uncatalogued"),
        "fixture did not create the uncatalogued reference"
    );
    assert!(
        !before
            .styles
            .page_styles
            .contains_key(&StyleId::new("Uncatalogued")),
        "fixture accidentally catalogued the name"
    );

    delete_page_style(&loro, "Uncatalogued").expect("delete");

    let after = loro_to_document(&loro).expect("read back");
    assert!(
        after.sections[0].page_style.is_none(),
        "the uncatalogued reference survived"
    );
}

/// The inverse: deleting a name nothing uses changes nothing, and deleting one
/// style leaves the others alone.
#[test]
fn delete_is_a_no_op_for_an_unknown_name_and_spares_the_others() {
    let loro = two_section_doc();
    create_page_style(&loro, "Keep", &landscape_letter()).expect("create");
    let before = format!("{:?}", loro_to_document(&loro).expect("read back"));

    delete_page_style(&loro, "NoSuchStyle").expect("delete unknown");
    assert_eq!(
        before,
        format!("{:?}", loro_to_document(&loro).expect("read back")),
        "deleting an unknown name changed the document"
    );

    delete_page_style(&loro, "PageStyle1").expect("delete");
    let after = loro_to_document(&loro).expect("read back");
    assert!(
        after.styles.page_styles.contains_key(&StyleId::new("Keep")),
        "deleting one style removed another"
    );
}
