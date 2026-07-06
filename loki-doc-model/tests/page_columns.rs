// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for page-column mutations — the model primitive behind the Layout
//! tab's column presets (plan 4a.2).

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::{document_column_count, set_document_columns};
use loro::LoroDoc;

fn doc() -> LoroDoc {
    let mut d = Document::new();
    d.sections[0].blocks = vec![Block::Para(vec![Inline::Str("hi".into())])];
    document_to_loro(&d).expect("to loro")
}

/// Section 0's reconstructed `SectionColumns` (count, gap, separator).
fn cols(loro: &LoroDoc) -> Option<(u8, f64, bool)> {
    let d = loro_to_document(loro).expect("rebuild");
    d.sections[0]
        .layout
        .columns
        .as_ref()
        .map(|c| (c.count, c.gap.value(), c.separator))
}

#[test]
fn a_fresh_document_is_single_column() {
    let loro = doc();
    assert_eq!(document_column_count(&loro), 1);
    assert!(cols(&loro).is_none(), "no columns map on a fresh doc");
}

#[test]
fn setting_two_columns_creates_a_gap() {
    let loro = doc();
    set_document_columns(&loro, 2).expect("two columns");
    assert_eq!(document_column_count(&loro), 2);
    let (count, gap, sep) = cols(&loro).expect("columns present");
    assert_eq!(count, 2);
    assert!(gap > 0.0, "a default gap is created ({gap})");
    assert!(!sep, "no separator line by default");
}

#[test]
fn changing_count_preserves_the_gap() {
    let loro = doc();
    set_document_columns(&loro, 2).expect("two");
    let gap2 = cols(&loro).unwrap().1;
    set_document_columns(&loro, 3).expect("three");
    let (count, gap3, _) = cols(&loro).unwrap();
    assert_eq!(count, 3);
    assert_eq!(gap2, gap3, "gap is preserved when the count changes");
}

#[test]
fn back_to_one_column_reads_as_single() {
    let loro = doc();
    set_document_columns(&loro, 3).expect("three");
    set_document_columns(&loro, 1).expect("one");
    assert_eq!(document_column_count(&loro), 1);
}

#[test]
fn zero_is_clamped_to_one() {
    let loro = doc();
    set_document_columns(&loro, 0).expect("clamped");
    assert_eq!(document_column_count(&loro), 1);
}
