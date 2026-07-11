// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for page-size mutations — the model primitive behind the Layout tab's
//! A4/Letter presets (plan 4a.2). Page size must preserve orientation.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::{document_page_size, set_document_orientation, set_document_page_size};
use loro::LoroDoc;

// A4 and US Letter in points (portrait).
const A4: (f64, f64) = (595.28, 841.89);
const LETTER: (f64, f64) = (612.0, 792.0);

fn doc() -> LoroDoc {
    let mut d = Document::new();
    d.sections[0].blocks = vec![Block::Para(vec![Inline::Str("hi".into())])];
    document_to_loro(&d).expect("to loro")
}

/// Section 0's `(width, height)` after a rebuild.
fn size(loro: &LoroDoc) -> (f64, f64) {
    let d = loro_to_document(loro).expect("rebuild");
    let s = &d.sections[0].layout.page_size;
    (s.width.value(), s.height.value())
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.5
}

#[test]
fn set_a4_on_a_portrait_doc_applies_portrait_a4() {
    let loro = doc();
    set_document_page_size(&loro, A4.0, A4.1).expect("A4");
    let (w, h) = size(&loro);
    assert!(
        close(w, A4.0) && close(h, A4.1),
        "portrait A4, got {w} x {h}"
    );
    assert_eq!(
        document_page_size(&loro).map(|(w, _)| close(w, A4.0)),
        Some(true)
    );
}

#[test]
fn page_size_preserves_landscape_orientation() {
    let loro = doc();
    set_document_orientation(&loro, true).expect("landscape");
    // Now pick A4: the long edge must stay the width (A4 landscape).
    set_document_page_size(&loro, A4.0, A4.1).expect("A4");
    let (w, h) = size(&loro);
    assert!(w > h, "still landscape after choosing A4 ({w} x {h})");
    assert!(close(w, A4.1) && close(h, A4.0), "A4 landscape dims");
}

#[test]
fn switching_letter_to_a4_changes_the_paper() {
    let loro = doc();
    // Fresh doc defaults to Letter.
    let (lw, lh) = size(&loro);
    assert!(close(lw, LETTER.0) && close(lh, LETTER.1), "starts Letter");

    set_document_page_size(&loro, A4.0, A4.1).expect("A4");
    let (w, h) = size(&loro);
    assert!(close(w, A4.0) && close(h, A4.1), "switched to A4");
    assert!(!close(w, LETTER.0), "no longer Letter width");
}
