// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! What reaches the document-colours group, and what is kept out of it.

use loki_doc_model::Document;
use loki_doc_model::ExtensionBag;
use loki_doc_model::loki_primitives::color::{CmykColor, DocumentColor, RgbColor, ThemeColorSlot};
use loki_doc_model::style::StyleId;
use loki_doc_model::style::char_style::CharacterStyle;
use loki_doc_model::style::props::char_props::CharProps;

use super::document_colors;

fn rgb(r: f32, g: f32, b: f32) -> DocumentColor {
    DocumentColor::Rgb(RgbColor::new(r, g, b))
}

fn doc_with(colors: Vec<Option<DocumentColor>>) -> Document {
    let mut doc = Document::default();
    for (i, c) in colors.into_iter().enumerate() {
        let id = StyleId::new(format!("s{i}"));
        doc.styles.character_styles.insert(
            id.clone(),
            CharacterStyle {
                id: id.clone(),
                display_name: None,
                parent: None,
                char_props: CharProps {
                    color: c,
                    ..CharProps::default()
                },
                extensions: ExtensionBag::default(),
            },
        );
    }
    doc
}

/// The ordinary case: each style's colour appears once, in catalog order.
#[test]
fn style_colours_appear_in_catalog_order() {
    let doc = doc_with(vec![
        Some(rgb(1.0, 0.0, 0.0)),
        Some(rgb(0.0, 1.0, 0.0)),
        Some(rgb(0.0, 0.0, 1.0)),
    ]);
    assert_eq!(document_colors(&doc), ["#FF0000", "#00FF00", "#0000FF"]);
}

/// **Distinct, not one entry per style.** Two styles in the same colour is the
/// common case — a heading and its run style — and a group listing it twice
/// wastes a swatch on a choice the reader already has.
#[test]
fn a_repeated_colour_appears_once() {
    let doc = doc_with(vec![
        Some(rgb(1.0, 0.0, 0.0)),
        Some(rgb(1.0, 0.0, 0.0)),
        Some(rgb(0.0, 1.0, 0.0)),
    ]);
    assert_eq!(document_colors(&doc), ["#FF0000", "#00FF00"]);
}

/// A style with no colour contributes nothing rather than a default.
#[test]
fn a_style_with_no_colour_contributes_nothing() {
    let doc = doc_with(vec![None, Some(rgb(1.0, 0.0, 0.0)), None]);
    assert_eq!(document_colors(&doc), ["#FF0000"]);
}

/// **A colour with no honest hex is left out.** CMYK has no sRGB value without
/// an ICC transform, and a theme reference resolves only against a `ThemeColor`
/// no importer in this workspace builds — so a swatch for either would show an
/// approximation of a colour the document never specified.
#[test]
fn colours_without_an_honest_hex_are_omitted() {
    let doc = doc_with(vec![
        Some(DocumentColor::Cmyk(CmykColor::new(0.0, 0.0, 0.0, 1.0))),
        Some(DocumentColor::Theme {
            slot: ThemeColorSlot::Accent1,
            tint: 0.0,
        }),
        Some(rgb(1.0, 0.0, 0.0)),
    ]);
    assert_eq!(
        document_colors(&doc),
        ["#FF0000"],
        "only the colour with a real hex survives",
    );
}

/// An empty document offers an empty group, so the caller can hide the section
/// rather than showing an empty heading.
#[test]
fn a_document_with_no_styles_offers_nothing() {
    assert!(document_colors(&Document::default()).is_empty());
}

/// **Capped, and the cap keeps the earliest.** A document with a large palette
/// would otherwise turn a group meant for scanning into one that needs
/// searching; keeping the *first* entries makes the list stable as the document
/// grows, where keeping the last would reshuffle it on every edit.
#[test]
fn the_group_is_capped_at_two_rows() {
    let many: Vec<_> = (0..30)
        .map(|i| Some(rgb(i as f32 / 30.0, 0.0, 0.0)))
        .collect();
    let out = document_colors(&doc_with(many));
    assert_eq!(out.len(), 12);
    assert_eq!(out[0], "#000000", "the first colour is kept");
}
