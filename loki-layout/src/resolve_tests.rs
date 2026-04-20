// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for [`crate::resolve`].

use super::*;

use appthere_color::RgbColor;
use loki_doc_model::content::attr::{ExtensionBag, NodeAttr};
use loki_doc_model::content::block::StyledParagraph;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::para_style::ParagraphStyle;
use loki_doc_model::style::props::char_props::{
    CharProps, HighlightColor,
    StrikethroughStyle as DocStrikethroughStyle,
    UnderlineStyle as DocUnderlineStyle,
    VerticalAlign as DocVerticalAlign,
};
use loki_doc_model::style::props::para_props::{ParagraphAlignment, ParaProps};
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;

// ── helpers ───────────────────────────────────────────────────────────────────

fn empty_para(inlines: Vec<Inline>) -> StyledParagraph {
    StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines,
        attr: NodeAttr::default(),
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[test]
fn resolve_color_rgb_values() {
    let dc = DocumentColor::Rgb(RgbColor::new(1.0, 0.5, 0.0));
    let lc = resolve_color(Some(&dc));
    assert!((lc.r - 1.0).abs() < 1e-5, "r mismatch");
    assert!((lc.g - 0.5).abs() < 1e-5, "g mismatch");
    assert!(lc.b.abs() < 1e-5, "b mismatch");
    assert!((lc.a - 1.0).abs() < 1e-5, "alpha should be 1.0");
}

#[test]
fn resolve_color_transparent() {
    let lc = resolve_color(Some(&DocumentColor::Transparent));
    assert_eq!(lc, LayoutColor::TRANSPARENT);
}

#[test]
fn resolve_color_none_gives_black() {
    assert_eq!(resolve_color(None), LayoutColor::BLACK);
}

#[test]
fn pts_to_f32_value() {
    let result = pts_to_f32(Points::new(14.5));
    assert!((result - 14.5_f32).abs() < 1e-5);
}

#[test]
fn flatten_plain_str() {
    let catalog = StyleCatalog::new();
    let para = empty_para(vec![Inline::Str("hello".into())]);
    let (text, spans) = flatten_paragraph(&para, &catalog);
    assert_eq!(text, "hello");
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].range, 0..5);
}

#[test]
fn flatten_str_space_str() {
    let catalog = StyleCatalog::new();
    let para = empty_para(vec![
        Inline::Str("hello".into()),
        Inline::Space,
        Inline::Str("world".into()),
    ]);
    let (text, _spans) = flatten_paragraph(&para, &catalog);
    assert_eq!(text, "hello world");
}

#[test]
fn flatten_strong_sets_bold() {
    let catalog = StyleCatalog::new();
    let para = empty_para(vec![Inline::Strong(vec![Inline::Str("bold".into())])]);
    let (text, spans) = flatten_paragraph(&para, &catalog);
    assert_eq!(text, "bold");
    assert!(!spans.is_empty());
    assert!(spans[0].bold, "Strong should produce bold=true");
}

#[test]
fn flatten_emph_sets_italic() {
    let catalog = StyleCatalog::new();
    let para = empty_para(vec![Inline::Emph(vec![Inline::Str("italic".into())])]);
    let (_, spans) = flatten_paragraph(&para, &catalog);
    assert!(!spans.is_empty());
    assert!(spans[0].italic, "Emph should produce italic=true");
}

#[test]
fn flatten_styled_run_applies_direct_props() {
    let catalog = StyleCatalog::new();
    let run = StyledRun {
        style_id: None,
        direct_props: Some(Box::new(CharProps {
            font_size: Some(Points::new(24.0)),
            bold: Some(true),
            ..Default::default()
        })),
        content: vec![Inline::Str("big".into())],
        attr: NodeAttr::default(),
    };
    let para = empty_para(vec![Inline::StyledRun(run)]);
    let (_, spans) = flatten_paragraph(&para, &catalog);
    assert!(!spans.is_empty());
    assert!((spans[0].font_size - 24.0).abs() < 1e-5, "font_size should be 24pt");
    assert!(spans[0].bold, "bold should be true");
}

#[test]
fn resolve_para_props_defaults() {
    let catalog = StyleCatalog::new();
    let para = empty_para(vec![]);
    let resolved = resolve_para_props(&para, &catalog);
    assert_eq!(resolved.space_before, 0.0);
    assert_eq!(resolved.indent_start, 0.0);
    assert!(!resolved.keep_together);
    assert!(!resolved.page_break_before);
}

#[test]
fn resolve_para_props_center_from_style() {
    let mut catalog = StyleCatalog::new();
    catalog.paragraph_styles.insert(
        StyleId::new("Center"),
        ParagraphStyle {
            id: StyleId::new("Center"),
            display_name: None,
            parent: None,
            linked_char_style: None,
            para_props: ParaProps {
                alignment: Some(ParagraphAlignment::Center),
                ..Default::default()
            },
            char_props: CharProps::default(),
            is_default: false,
            is_custom: false,
            extensions: ExtensionBag::default(),
        },
    );
    let para = StyledParagraph {
        style_id: Some(StyleId::new("Center")),
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![],
        attr: NodeAttr::default(),
    };
    let resolved = resolve_para_props(&para, &catalog);
    assert_eq!(resolved.alignment, parley::Alignment::Center);
}

#[test]
fn char_props_to_style_span_maps_new_fields() {
    let props = CharProps {
        vertical_align: Some(DocVerticalAlign::Superscript),
        highlight_color: Some(HighlightColor::Yellow),
        letter_spacing: Some(Points::new(2.0)),
        small_caps: Some(true),
        word_spacing: Some(Points::new(3.0)),
        shadow: Some(true),
        underline: Some(DocUnderlineStyle::Double),
        strikethrough: Some(DocStrikethroughStyle::Single),
        ..Default::default()
    };
    let span = char_props_to_style_span(&props, 0..1);

    assert_eq!(span.vertical_align, Some(crate::para::VerticalAlign::Superscript));
    assert!(span.highlight_color.is_some(), "highlight_color must be mapped");
    assert!((span.letter_spacing.unwrap() - 2.0).abs() < 1e-5);
    assert_eq!(span.font_variant, Some(crate::para::FontVariant::SmallCaps));
    assert!((span.word_spacing.unwrap() - 3.0).abs() < 1e-5);
    assert!(span.shadow, "shadow must be true");
    assert!(span.underline.is_some(), "underline must be mapped");
    assert!(span.strikethrough.is_some(), "strikethrough must be mapped");
}

#[test]
fn flatten_all_caps_uppercases_text() {
    let catalog = StyleCatalog::new();
    let run = StyledRun {
        style_id: None,
        direct_props: Some(Box::new(CharProps {
            all_caps: Some(true),
            ..Default::default()
        })),
        content: vec![Inline::Str("hello".into())],
        attr: NodeAttr::default(),
    };
    let para = empty_para(vec![Inline::StyledRun(run)]);
    let (text, spans) = flatten_paragraph(&para, &catalog);
    assert_eq!(text, "HELLO", "all_caps must uppercase text during flatten");
    assert_eq!(spans[0].font_variant, Some(crate::para::FontVariant::AllCaps));
}

#[test]
fn flatten_superscript_inline_sets_vertical_align() {
    let catalog = StyleCatalog::new();
    let para = empty_para(vec![Inline::Superscript(vec![Inline::Str("2".into())])]);
    let (text, spans) = flatten_paragraph(&para, &catalog);
    assert_eq!(text, "2");
    assert_eq!(
        spans[0].vertical_align,
        Some(crate::para::VerticalAlign::Superscript),
        "Inline::Superscript must set vertical_align=Superscript"
    );
}
