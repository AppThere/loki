// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for page-orientation mutations — the model primitive behind the
//! Layout tab's orientation toggle (plan 4a.2).

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::PageOrientation;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::{document_is_landscape, set_document_orientation};
use loro::LoroDoc;

fn portrait_doc() -> LoroDoc {
    // `Document::new()` seeds an A4 portrait section (width < height).
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(vec![Inline::Str("hi".into())])];
    document_to_loro(&doc).expect("to loro")
}

/// `(width, height, orientation)` of section 0 after a rebuild.
fn page(loro: &LoroDoc) -> (f64, f64, PageOrientation) {
    let doc = loro_to_document(loro).expect("rebuild");
    let layout = &doc.sections[0].layout;
    (
        layout.page_size.width.value(),
        layout.page_size.height.value(),
        layout.orientation,
    )
}

#[test]
fn a_fresh_document_is_portrait() {
    let loro = portrait_doc();
    assert!(!document_is_landscape(&loro));
    let (w, h, _) = page(&loro);
    assert!(w < h, "portrait page is taller than wide ({w} x {h})");
}

#[test]
fn to_landscape_swaps_dimensions_and_sets_the_flag() {
    let loro = portrait_doc();
    let (pw, ph, _) = page(&loro);

    set_document_orientation(&loro, true).expect("landscape");
    assert!(document_is_landscape(&loro));

    let (w, h, orient) = page(&loro);
    assert!((w - ph).abs() < 1e-6, "new width is the old height");
    assert!((h - pw).abs() < 1e-6, "new height is the old width");
    assert!(w > h, "landscape page is wider than tall");
    assert_eq!(orient, PageOrientation::Landscape);
}

#[test]
fn toggling_back_to_portrait_restores_dimensions() {
    let loro = portrait_doc();
    let before = page(&loro);
    set_document_orientation(&loro, true).expect("landscape");
    set_document_orientation(&loro, false).expect("portrait");
    let after = page(&loro);
    assert!((before.0 - after.0).abs() < 1e-6, "width restored");
    assert!((before.1 - after.1).abs() < 1e-6, "height restored");
    assert_eq!(after.2, PageOrientation::Portrait);
}

#[test]
fn setting_the_same_orientation_is_idempotent() {
    let loro = portrait_doc();
    let before = page(&loro);
    // Already portrait → no dimension change, flag set to Portrait.
    set_document_orientation(&loro, false).expect("portrait");
    let after = page(&loro);
    assert_eq!(before.0, after.0);
    assert_eq!(before.1, after.1);
    assert!(!document_is_landscape(&loro));
}
