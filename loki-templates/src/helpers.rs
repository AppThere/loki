// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared builder helpers for the bundled templates.
//!
//! Templates are authored using only the properties that the DOCX exporter and
//! importer round-trip faithfully (font, size, bold/italic/underline,
//! alignment, indentation, spacing, line height, and named styles), so the
//! generated `.dotx` assets re-import losslessly.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::layout::page::{PageLayout, PageMargins, PageSize};
use loki_doc_model::layout::section::Section;
use loki_doc_model::meta::DocumentMeta;
use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::para_style::ParagraphStyle;
use loki_doc_model::style::props::char_props::{CharProps, UnderlineStyle};
use loki_doc_model::style::props::para_props::{
    LineHeight, ParaProps, ParagraphAlignment, Spacing,
};
use loki_primitives::units::Points;

/// Points in `v` inches (1 in = 72 pt).
#[must_use]
pub(crate) fn inches(v: f64) -> Points {
    Points::new(v * 72.0)
}

/// A US-Letter page layout with uniform `margin_in`-inch margins.
#[must_use]
pub(crate) fn letter_layout(margin_in: f64) -> PageLayout {
    let m = inches(margin_in);
    PageLayout {
        page_size: PageSize::letter(),
        margins: PageMargins {
            top: m,
            bottom: m,
            left: m,
            right: m,
            header: inches(0.5),
            footer: inches(0.5),
            gutter: Points::new(0.0),
        },
        ..PageLayout::default()
    }
}

/// Character-formatting spec for a style (only round-trip-safe properties).
#[derive(Default, Clone)]
pub(crate) struct Char {
    pub font: Option<&'static str>,
    pub size: Option<f64>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

impl Char {
    fn to_props(&self) -> CharProps {
        CharProps {
            font_name: self.font.map(str::to_string),
            font_size: self.size.map(Points::new),
            bold: self.bold.then_some(true),
            italic: self.italic.then_some(true),
            underline: self.underline.then_some(UnderlineStyle::Single),
            ..Default::default()
        }
    }
}

/// Paragraph-formatting spec for a style (measurements in points; `line` is a
/// line-spacing ratio, e.g. `2.0` for double spacing).
#[derive(Default, Clone)]
pub(crate) struct Para {
    pub align: Option<ParagraphAlignment>,
    pub indent_first: Option<f64>,
    pub indent_left: Option<f64>,
    pub hanging: Option<f64>,
    pub space_before: Option<f64>,
    pub space_after: Option<f64>,
    pub line: Option<f32>,
    pub outline: Option<u8>,
}

impl Para {
    fn to_props(&self) -> ParaProps {
        ParaProps {
            alignment: self.align,
            indent_first_line: self.indent_first.map(Points::new),
            indent_start: self.indent_left.map(Points::new),
            indent_hanging: self.hanging.map(Points::new),
            space_before: self.space_before.map(|v| Spacing::Exact(Points::new(v))),
            space_after: self.space_after.map(|v| Spacing::Exact(Points::new(v))),
            line_height: self.line.map(LineHeight::Multiple),
            outline_level: self.outline,
            ..Default::default()
        }
    }
}

/// Builds a named paragraph style. `Normal` and `Heading1`–`Heading6` are marked
/// built-in; every other id is a custom style.
#[must_use]
pub(crate) fn style(
    id: &str,
    name: &str,
    parent: Option<&str>,
    next: Option<&str>,
    ch: &Char,
    pa: &Para,
) -> ParagraphStyle {
    let is_builtin = id == "Normal" || (id.starts_with("Heading") && id.len() == 8);
    ParagraphStyle {
        id: StyleId::new(id),
        display_name: Some(name.to_string()),
        parent: parent.map(StyleId::new),
        linked_char_style: None,
        next_style_id: next.map(str::to_string),
        para_props: pa.to_props(),
        char_props: ch.to_props(),
        is_default: id == "Normal",
        is_custom: !is_builtin,
        extensions: Default::default(),
    }
}

/// A single plain-text inline run.
#[must_use]
pub(crate) fn inline(s: &str) -> Vec<Inline> {
    vec![Inline::Str(s.to_string())]
}

/// A semantic heading block at `level` (1–6) with plain text `s`. Renders via
/// the catalog's `Heading{level}` style.
#[must_use]
pub(crate) fn heading_block(level: u8, s: &str) -> Block {
    Block::Heading(level, NodeAttr::default(), inline(s))
}

/// A paragraph carrying named style `style_id` with plain text `s` (empty `s`
/// yields an empty paragraph used for vertical spacing).
#[must_use]
pub(crate) fn p(style_id: &str, s: &str) -> Block {
    Block::StyledPara(StyledParagraph {
        style_id: Some(StyleId::new(style_id)),
        direct_para_props: None,
        direct_char_props: None,
        inlines: if s.is_empty() {
            vec![]
        } else {
            vec![Inline::Str(s.to_string())]
        },
        attr: NodeAttr::default(),
    })
}

/// Assembles a single-section [`Document`] from a title, layout, styles, and body.
#[must_use]
pub(crate) fn assemble(
    title: &str,
    layout: PageLayout,
    styles: Vec<ParagraphStyle>,
    blocks: Vec<Block>,
) -> Document {
    let mut catalog = StyleCatalog::new();
    for s in styles {
        catalog.paragraph_styles.insert(s.id.clone(), s);
    }
    Document {
        meta: DocumentMeta {
            title: Some(title.to_string()),
            ..Default::default()
        },
        styles: catalog,
        sections: vec![Section::with_layout_and_blocks(layout, blocks)],
        settings: None,
        comments: Vec::new(),
        source: None,
    }
}
