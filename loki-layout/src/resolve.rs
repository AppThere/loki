// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Style resolution — bridges `loki-doc-model` types to the renderer-agnostic
//! layout types.
//!
//! The public functions take a [`StyledParagraph`] / [`StyledRun`] plus a
//! [`StyleCatalog`] and produce the flattened representations consumed by
//! [`crate::para::layout_paragraph`].

use std::ops::Range;

use loki_doc_model::content::block::StyledParagraph;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::props::border::{Border as DocBorder, BorderStyle as DocBorderStyle};
use loki_doc_model::style::props::char_props::{CharProps, StrikethroughStyle, UnderlineStyle};
use loki_doc_model::style::props::para_props::{
    LineHeight as DocLineHeight, ParagraphAlignment, ParaProps, Spacing,
};
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;
use parley::Alignment;

use crate::color::LayoutColor;
use crate::geometry::LayoutInsets;
use crate::items::{BorderEdge, BorderStyle};
use crate::para::{ResolvedLineHeight, ResolvedParaProps, StyleSpan};

// ── Public API ────────────────────────────────────────────────────────────────

/// Convert an optional [`DocumentColor`] to a [`LayoutColor`].
///
/// - `None` → [`LayoutColor::BLACK`] (default text colour).
/// - `Rgb(c)` → linear sRGB via [`LayoutColor::from`].
/// - `Transparent` → [`LayoutColor::TRANSPARENT`].
/// - `Cmyk`, `Theme`, and any future variants → [`LayoutColor::BLACK`]
///   (no ICC transform or theme resolver is available at layout time).
pub fn resolve_color(color: Option<&DocumentColor>) -> LayoutColor {
    match color {
        None => LayoutColor::BLACK,
        Some(DocumentColor::Transparent) => LayoutColor::TRANSPARENT,
        Some(DocumentColor::Rgb(rgb)) => LayoutColor::from(*rgb),
        Some(_) => LayoutColor::BLACK,
    }
}

/// Convert a [`Points`] value to `f32`.
pub fn pts_to_f32(pts: Points) -> f32 {
    pts.value() as f32
}

/// Resolve the effective [`ResolvedParaProps`] for a [`StyledParagraph`].
///
/// Resolution order (child wins):
/// 1. Named style chain via [`StyleCatalog::resolve_para`].
/// 2. Direct paragraph formatting on the paragraph itself.
pub fn resolve_para_props(block: &StyledParagraph, catalog: &StyleCatalog) -> ResolvedParaProps {
    let mut base: ParaProps = block
        .style_id
        .as_ref()
        .and_then(|id| catalog.resolve_para(id))
        .unwrap_or_default();
    if let Some(direct) = &block.direct_para_props {
        base = direct.as_ref().clone().merged_with_parent(&base);
    }
    map_para_props(&base)
}

/// Resolve the effective [`StyleSpan`] properties for a [`StyledRun`].
///
/// Resolution order (child wins):
/// 1. `para_char_defaults` (paragraph's resolved character properties).
/// 2. Character style chain from `run.style_id`.
/// 3. Direct run formatting.
///
/// The returned span has `range: 0..0`. Callers should overwrite `range` with
/// the actual byte positions of the run's text in the flattened paragraph string.
pub fn resolve_char_props(
    run: &StyledRun,
    catalog: &StyleCatalog,
    para_char_defaults: &CharProps,
) -> StyleSpan {
    char_props_to_style_span(&effective_run_char_props(run, catalog, para_char_defaults), 0..0)
}

/// Flatten all inline content of a [`StyledParagraph`] into a UTF-8 string
/// and a matching list of [`StyleSpan`]s.
///
/// Each span's `range` is a byte range within the returned string.
pub fn flatten_paragraph(
    block: &StyledParagraph,
    catalog: &StyleCatalog,
) -> (String, Vec<StyleSpan>) {
    let base: CharProps = block
        .style_id
        .as_ref()
        .and_then(|id| catalog.resolve_char(id))
        .unwrap_or_default();
    let base = match &block.direct_char_props {
        Some(direct) => direct.as_ref().clone().merged_with_parent(&base),
        None => base,
    };
    let mut buf = String::new();
    let mut spans: Vec<StyleSpan> = Vec::new();
    walk_inlines(&block.inlines, &base, catalog, &mut buf, &mut spans);
    (buf, spans)
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Walk the character-style parent chain in [`StyleCatalog::character_styles`].
fn resolve_char_style_chain(catalog: &StyleCatalog, id: &StyleId) -> CharProps {
    let Some(style) = catalog.character_styles.get(id) else {
        return CharProps::default();
    };
    let own = style.char_props.clone();
    if let Some(ref parent_id) = style.parent {
        let parent = resolve_char_style_chain(catalog, parent_id);
        own.merged_with_parent(&parent)
    } else {
        own
    }
}

/// Compute the effective [`CharProps`] for a run (3-layer merge).
fn effective_run_char_props(
    run: &StyledRun,
    catalog: &StyleCatalog,
    parent: &CharProps,
) -> CharProps {
    let mut props = parent.clone();
    if let Some(ref id) = run.style_id {
        props = resolve_char_style_chain(catalog, id).merged_with_parent(&props);
    }
    if let Some(ref direct) = run.direct_props {
        props = direct.as_ref().clone().merged_with_parent(&props);
    }
    props
}

/// Convert a [`CharProps`] snapshot to a [`StyleSpan`] covering `range`.
fn char_props_to_style_span(props: &CharProps, range: Range<usize>) -> StyleSpan {
    StyleSpan {
        range,
        font_name: props.font_name.clone(),
        font_size: props.font_size.map(pts_to_f32).unwrap_or(12.0),
        bold: props.bold.unwrap_or(false),
        italic: props.italic.unwrap_or(false),
        color: resolve_color(props.color.as_ref()),
        underline: props.underline.is_some(),
        strikethrough: props.strikethrough.is_some(),
        line_height: None,
    }
}

/// Append `text` to `buf` and push a span; no-op for empty strings.
#[inline]
fn push_text(buf: &mut String, spans: &mut Vec<StyleSpan>, text: &str, props: &CharProps) {
    if text.is_empty() {
        return;
    }
    let start = buf.len();
    buf.push_str(text);
    spans.push(char_props_to_style_span(props, start..buf.len()));
}

/// Recursively collect text from an [`Inline`] tree, building `buf` + `spans`.
fn walk_inlines(
    inlines: &[Inline],
    effective: &CharProps,
    catalog: &StyleCatalog,
    buf: &mut String,
    spans: &mut Vec<StyleSpan>,
) {
    for inline in inlines {
        match inline {
            Inline::Str(s) => push_text(buf, spans, s, effective),
            Inline::Space => push_text(buf, spans, " ", effective),
            Inline::SoftBreak => push_text(buf, spans, " ", effective),
            Inline::LineBreak => push_text(buf, spans, "\n", effective),
            Inline::Code(_, s) => push_text(buf, spans, s, effective),
            Inline::StyledRun(run) => {
                let p = effective_run_char_props(run, catalog, effective);
                walk_inlines(&run.content, &p, catalog, buf, spans);
            }
            Inline::Strong(ch) => {
                let mut p = effective.clone();
                p.bold = Some(true);
                walk_inlines(ch, &p, catalog, buf, spans);
            }
            Inline::Emph(ch) => {
                let mut p = effective.clone();
                p.italic = Some(true);
                walk_inlines(ch, &p, catalog, buf, spans);
            }
            Inline::Underline(ch) => {
                let mut p = effective.clone();
                p.underline = Some(UnderlineStyle::Single);
                walk_inlines(ch, &p, catalog, buf, spans);
            }
            Inline::Strikeout(ch) => {
                let mut p = effective.clone();
                p.strikethrough = Some(StrikethroughStyle::Single);
                walk_inlines(ch, &p, catalog, buf, spans);
            }
            Inline::Quoted(_, ch) | Inline::Span(_, ch) => {
                walk_inlines(ch, effective, catalog, buf, spans);
            }
            Inline::SmallCaps(ch) | Inline::Superscript(ch) | Inline::Subscript(ch) => {
                walk_inlines(ch, effective, catalog, buf, spans);
            }
            Inline::Link(_, ch, _) | Inline::Image(_, ch, _) => {
                walk_inlines(ch, effective, catalog, buf, spans);
            }
            Inline::Cite(_, ch) => walk_inlines(ch, effective, catalog, buf, spans),
            // Math, RawInline, Note, Field, Comment, Bookmark, and any
            // future #[non_exhaustive] variants are not text runs — skip.
            _ => {}
        }
    }
}

/// Map a doc [`Border`][DocBorder] to a layout [`BorderEdge`], or `None` when
/// the border style is [`DocBorderStyle::None`].
fn convert_border(border: &DocBorder) -> Option<BorderEdge> {
    if border.style == DocBorderStyle::None {
        return None;
    }
    Some(BorderEdge {
        color: resolve_color(border.color.as_ref()),
        width: pts_to_f32(border.width),
        style: match border.style {
            DocBorderStyle::Dashed => BorderStyle::Dashed,
            DocBorderStyle::Dotted => BorderStyle::Dotted,
            DocBorderStyle::Double => BorderStyle::Double,
            _ => BorderStyle::Solid,
        },
    })
}

/// Map a [`Spacing`] variant to a point value; percentage-based spacing
/// falls back to `0.0` (line height is not known at this stage).
#[inline]
fn resolve_spacing(s: Option<Spacing>) -> f32 {
    match s {
        Some(Spacing::Exact(pts)) => pts_to_f32(pts),
        _ => 0.0,
    }
}

/// Map a [`ParaProps`] record to the layout [`ResolvedParaProps`].
fn map_para_props(p: &ParaProps) -> ResolvedParaProps {
    ResolvedParaProps {
        alignment: match p.alignment {
            Some(ParagraphAlignment::Right) => Alignment::End,
            Some(ParagraphAlignment::Center) => Alignment::Center,
            Some(ParagraphAlignment::Justify) => Alignment::Justify,
            _ => Alignment::Start,
        },
        space_before: resolve_spacing(p.space_before),
        space_after: resolve_spacing(p.space_after),
        indent_start: p.indent_start.map(pts_to_f32).unwrap_or(0.0),
        indent_end: p.indent_end.map(pts_to_f32).unwrap_or(0.0),
        indent_first_line: p.indent_first_line.map(pts_to_f32).unwrap_or(0.0),
        line_height: p.line_height.and_then(|lh| match lh {
            // IMPORTANT: The OOXML mapper stores Multiple as a ratio, NOT a
            // percentage, despite the doc-model comment (e.g. line=240 →
            // Multiple(1.0), line=360 → Multiple(1.5)). Do NOT divide by 100.
            //
            // lineRule="auto" with line=240 (single spacing) is the most common
            // case. Return None so Parley uses natural font metrics
            // (ascender + descender + leading — exactly what "auto" means).
            // For non-unity multipliers, MetricsRelative scales those natural
            // metrics (1.5 = one-and-a-half spacing, 2.0 = double spacing).
            DocLineHeight::Multiple(m) => {
                if (m - 1.0).abs() < 0.02 {
                    None // Single spacing — let Parley default take over
                } else {
                    Some(ResolvedLineHeight::MetricsRelative(m))
                }
            }
            DocLineHeight::Exact(pts) => Some(ResolvedLineHeight::Exact(pts_to_f32(pts))),
            DocLineHeight::AtLeast(pts) => Some(ResolvedLineHeight::AtLeast(pts_to_f32(pts))),
            // Future variants — fall back to natural metrics.
            _ => None,
        }),
        background_color: p.background_color.as_ref().map(|c| resolve_color(Some(c))),
        border_top: p.border_top.as_ref().and_then(convert_border),
        border_bottom: p.border_bottom.as_ref().and_then(convert_border),
        border_left: p.border_left.as_ref().and_then(convert_border),
        border_right: p.border_right.as_ref().and_then(convert_border),
        padding: LayoutInsets {
            top: p.padding_top.map(pts_to_f32).unwrap_or(0.0),
            right: p.padding_right.map(pts_to_f32).unwrap_or(0.0),
            bottom: p.padding_bottom.map(pts_to_f32).unwrap_or(0.0),
            left: p.padding_left.map(pts_to_f32).unwrap_or(0.0),
        },
        keep_together: p.keep_together.unwrap_or(false),
        keep_with_next: p.keep_with_next.unwrap_or(false),
        page_break_before: p.page_break_before.unwrap_or(false),
    }
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
