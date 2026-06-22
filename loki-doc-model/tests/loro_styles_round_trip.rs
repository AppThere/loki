// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Verifies the style catalog round-trips through the Loro CRDT bridge
//! (`document_to_loro` → `loro_to_document`) and that an in-place
//! `write_document_styles` mutation is picked up on the next re-derive — the
//! mechanism that makes style-editor edits durable and undoable.

use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document, write_document_styles};
use loki_doc_model::style::ParagraphStyle;
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::{ParaProps, ParagraphAlignment};
use loki_doc_model::{Document, loki_primitives::units::Points};

fn custom_style(id: &str, size: f64) -> ParagraphStyle {
    ParagraphStyle {
        id: StyleId::new(id),
        display_name: Some(format!("{id} Display")),
        parent: Some(StyleId::new("Normal")),
        linked_char_style: None,
        next_style_id: Some("Normal".into()),
        para_props: ParaProps {
            alignment: Some(ParagraphAlignment::Justify),
            indent_start: Some(Points::new(36.0)),
            ..Default::default()
        },
        char_props: CharProps {
            font_name: Some("Arial".into()),
            font_size: Some(Points::new(size)),
            bold: Some(true),
            ..Default::default()
        },
        is_default: false,
        is_custom: true,
        extensions: Default::default(),
    }
}

#[test]
fn catalog_survives_document_to_loro_round_trip() {
    let mut doc = Document::new();
    doc.styles
        .paragraph_styles
        .insert(StyleId::new("MyQuote"), custom_style("MyQuote", 14.0));

    let loro = document_to_loro(&doc).expect("to loro");
    let restored = loro_to_document(&loro).expect("from loro");

    let s = restored
        .styles
        .paragraph_styles
        .get(&StyleId::new("MyQuote"))
        .expect("custom style must survive the Loro round-trip");
    assert_eq!(s.display_name.as_deref(), Some("MyQuote Display"));
    assert_eq!(s.char_props.font_size, Some(Points::new(14.0)));
    assert!(s.is_custom);
}

#[test]
fn in_place_style_mutation_is_rederived() {
    let doc = Document::new();
    let loro = document_to_loro(&doc).expect("to loro");

    // Simulate a style-editor Apply: clone the catalog, add a style, write it
    // back to Loro, and commit (the editor's `commit_style_to_loro` path).
    let mut catalog = loro_to_document(&loro).expect("derive").styles;
    catalog
        .paragraph_styles
        .insert(StyleId::new("Callout"), custom_style("Callout", 18.0));
    write_document_styles(&loro, &catalog).expect("write styles");
    loro.commit();

    // A fresh re-derive (what `apply_mutation_and_relayout` does) sees the edit.
    let restored = loro_to_document(&loro).expect("re-derive");
    assert!(
        restored
            .styles
            .paragraph_styles
            .contains_key(&StyleId::new("Callout")),
        "style written via write_document_styles must appear on re-derive"
    );
}
