// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Unit tests for [`crate::resolve`].

use super::*;

use appthere_color::RgbColor;
use loki_doc_model::content::attr::{ExtensionBag, NodeAttr};
use loki_doc_model::content::block::StyledParagraph;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::para_style::ParagraphStyle;
use loki_doc_model::style::props::char_props::CharProps;
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
