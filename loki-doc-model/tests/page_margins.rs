// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for page-margin mutations — the model primitive behind the Layout
//! tab's margin presets (plan 4a.2).

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::{document_margins, set_document_margins};
use loro::LoroDoc;

fn doc() -> LoroDoc {
    let mut d = Document::new();
    d.sections[0].blocks = vec![Block::Para(vec![Inline::Str("hi".into())])];
    document_to_loro(&d).expect("to loro")
}

/// Section 0's `(top, bottom, left, right)` margins after a rebuild.
fn margins(loro: &LoroDoc) -> (f64, f64, f64, f64) {
    let d = loro_to_document(loro).expect("rebuild");
    let m = &d.sections[0].layout.margins;
    (
        m.top.value(),
        m.bottom.value(),
        m.left.value(),
        m.right.value(),
    )
}

#[test]
fn set_margins_updates_all_four_edges() {
    let loro = doc();
    // "Narrow": 36pt (0.5in) all round.
    set_document_margins(&loro, 36.0, 36.0, 36.0, 36.0).expect("set");
    assert_eq!(margins(&loro), (36.0, 36.0, 36.0, 36.0));
    assert_eq!(document_margins(&loro), Some((36.0, 36.0, 36.0, 36.0)));
}

#[test]
fn set_margins_can_differ_per_edge() {
    let loro = doc();
    // "Wide": 72pt top/bottom, 144pt left/right.
    set_document_margins(&loro, 72.0, 72.0, 144.0, 144.0).expect("set");
    assert_eq!(margins(&loro), (72.0, 72.0, 144.0, 144.0));
}

#[test]
fn header_footer_distances_are_left_untouched() {
    let loro = doc();
    let before = loro_to_document(&loro).expect("rebuild").sections[0]
        .layout
        .margins
        .header
        .value();
    set_document_margins(&loro, 20.0, 20.0, 20.0, 20.0).expect("set");
    let after = loro_to_document(&loro).expect("rebuild").sections[0]
        .layout
        .margins
        .header
        .value();
    assert_eq!(before, after, "header distance must not change");
}
