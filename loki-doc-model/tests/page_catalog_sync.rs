// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The document-wide page mutations must keep referenced page styles' catalog
//! copies in step with the sections ("both copies or neither", the
//! `loro_mutation::page_style` invariant).
//!
//! The defect these guard against: a ribbon geometry edit updated only the
//! sections, so the page dialog — which seeds its draft from the catalog copy —
//! showed the pre-ribbon geometry and re-committed it on Apply, silently
//! reverting the ribbon edit.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::{PageLayout, PageOrientation, PageSize};
use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::{
    create_page_style, set_document_margins, set_document_orientation, set_document_page_size,
};
use loro::LoroDoc;

// A4 in points (portrait) — distinguishable from the default US Letter.
const A4: (f64, f64) = (595.28, 841.89);

/// A one-section document whose section references a catalogued page style.
fn doc_with_style_ref() -> LoroDoc {
    let mut d = Document::new();
    d.sections[0].blocks = vec![Block::Para(vec![Inline::Str("hi".into())])];
    d.assign_page_styles();
    assert!(
        d.sections[0].page_style.is_some(),
        "setup: the section must reference a page style"
    );
    document_to_loro(&d).expect("to loro")
}

/// The catalog copy of the style section 0 references, after a rebuild.
fn referenced_catalog_layout(loro: &LoroDoc) -> PageLayout {
    let d = loro_to_document(loro).expect("rebuild");
    let id = d.sections[0]
        .page_style
        .clone()
        .expect("section still references its style");
    d.styles
        .page_styles
        .get(&id)
        .expect("catalog copy exists")
        .layout
        .clone()
}

fn close(a: Points, x: f64) -> bool {
    (a.value() - x).abs() < 0.5
}

#[test]
fn document_page_size_change_updates_the_catalog_copy() {
    let loro = doc_with_style_ref();
    set_document_page_size(&loro, A4.0, A4.1).expect("A4");

    let layout = referenced_catalog_layout(&loro);
    assert!(
        close(layout.page_size.width, A4.0) && close(layout.page_size.height, A4.1),
        "catalog copy follows the ribbon edit, got {:?}",
        layout.page_size
    );
}

#[test]
fn document_margin_change_updates_the_catalog_copy() {
    let loro = doc_with_style_ref();
    set_document_margins(&loro, 36.0, 36.0, 36.0, 36.0).expect("narrow");

    let m = referenced_catalog_layout(&loro).margins;
    assert!(
        close(m.top, 36.0) && close(m.bottom, 36.0) && close(m.left, 36.0) && close(m.right, 36.0),
        "catalog margins follow the ribbon edit, got {m:?}"
    );
}

#[test]
fn document_orientation_change_updates_the_catalog_copy() {
    let loro = doc_with_style_ref();
    set_document_orientation(&loro, true).expect("landscape");

    let layout = referenced_catalog_layout(&loro);
    assert_eq!(layout.orientation, PageOrientation::Landscape);
    assert!(
        layout.page_size.width.value() > layout.page_size.height.value(),
        "catalog size swapped with the sections"
    );
}

/// Guard inversion: a page style **no section references** has no section to
/// speak for it — a document-wide mutation must leave its catalog entry alone.
#[test]
fn unreferenced_style_is_not_touched() {
    let loro = doc_with_style_ref();
    let landscape_letter = PageLayout {
        page_size: PageSize {
            width: Points::new(792.0),
            height: Points::new(612.0),
        },
        orientation: PageOrientation::Landscape,
        ..Default::default()
    };
    create_page_style(&loro, "Unused", &landscape_letter).expect("create");

    set_document_page_size(&loro, A4.0, A4.1).expect("A4");

    let d = loro_to_document(&loro).expect("rebuild");
    let unused = d
        .styles
        .page_styles
        .get(&StyleId::new("Unused"))
        .expect("still catalogued");
    assert_eq!(
        unused.layout.orientation,
        PageOrientation::Landscape,
        "unreferenced style keeps its own definition"
    );
    assert!(
        close(unused.layout.page_size.width, 792.0),
        "unreferenced style keeps its own size"
    );
}
