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

//! Paragraph-level layout using Parley.
//!
//! [`layout_paragraph`] takes a flattened text string with ranged
//! [`StyleSpan`]s and paragraph properties, runs Parley shaping and
//! line-breaking, then converts the result into renderer-agnostic
//! [`PositionedItem`]s whose origins are relative to the paragraph's
//! own `(0, 0)` top-left corner.

use std::ops::Range;
use std::sync::Arc;

use parley::{
    Alignment, AlignmentOptions, FontFamily, FontStyle, FontWeight, LineHeight,
    PositionedLayoutItem, StyleProperty,
};

use crate::color::LayoutColor;
use crate::font::FontResources;
use crate::geometry::{LayoutInsets, LayoutPoint, LayoutRect};
use crate::items::{
    BorderEdge, DecorationKind, GlyphEntry, GlyphSynthesis, PositionedBorderRect,
    PositionedDecoration, PositionedGlyphRun, PositionedItem, PositionedRect,
};

/// Character-level style applied to a byte range within the paragraph text.
#[derive(Debug, Clone)]
pub struct StyleSpan {
    /// Byte range within the flattened text string.
    pub range: Range<usize>,
    /// Named font family override, or `None` to use the document default.
    pub font_name: Option<String>,
    /// Font size in points.
    pub font_size: f32,
    /// Bold weight.
    pub bold: bool,
    /// Italic style.
    pub italic: bool,
    /// Text colour.
    pub color: LayoutColor,
    /// Draw an underline.
    pub underline: bool,
    /// Draw a strikethrough.
    pub strikethrough: bool,
    /// Line-height multiplier (e.g. `1.5`). `None` = paragraph default.
    pub line_height: Option<f32>,
}

/// Resolved paragraph-level properties passed to [`layout_paragraph`].
#[derive(Debug, Clone)]
pub struct ResolvedParaProps {
    /// Horizontal text alignment.
    pub alignment: Alignment,
    /// Space above this paragraph in points (handled by the caller, not
    /// included in [`ParagraphLayout::height`]).
    pub space_before: f32,
    /// Space below this paragraph in points (handled by the caller).
    pub space_after: f32,
    /// Left indent in points.
    pub indent_start: f32,
    /// Right indent in points.
    pub indent_end: f32,
    /// First-line additional indent in points.
    pub indent_first_line: f32,
    /// Paragraph-level line-height multiplier override.
    pub line_height: Option<f32>,
    /// Optional paragraph background fill.
    pub background_color: Option<LayoutColor>,
    /// Top border edge, or `None`.
    pub border_top: Option<BorderEdge>,
    /// Bottom border edge, or `None`.
    pub border_bottom: Option<BorderEdge>,
    /// Left border edge, or `None`.
    pub border_left: Option<BorderEdge>,
    /// Right border edge, or `None`.
    pub border_right: Option<BorderEdge>,
    /// Internal padding inside the paragraph box.
    pub padding: LayoutInsets,
    /// Attempt to keep all lines of this paragraph on one page.
    pub keep_together: bool,
    /// Keep this paragraph on the same page as the next.
    pub keep_with_next: bool,
    /// Insert a page break before this paragraph.
    pub page_break_before: bool,
}

impl Default for ResolvedParaProps {
    fn default() -> Self {
        Self {
            alignment: Alignment::Start,
            space_before: 0.0,
            space_after: 0.0,
            indent_start: 0.0,
            indent_end: 0.0,
            indent_first_line: 0.0,
            line_height: None,
            background_color: None,
            border_top: None,
            border_bottom: None,
            border_left: None,
            border_right: None,
            padding: LayoutInsets::default(),
            keep_together: false,
            keep_with_next: false,
            page_break_before: false,
        }
    }
}

/// The measured result of laying out one paragraph.
#[derive(Debug, Clone)]
pub struct ParagraphLayout {
    /// Total height of this paragraph including internal line spacing.
    /// Does **not** include [`ResolvedParaProps::space_before`] /
    /// [`ResolvedParaProps::space_after`]; those are for the caller.
    pub height: f32,
    /// Maximum line width used (≤ `available_width`).
    pub width: f32,
    /// Positioned items from this paragraph (glyph runs + decorations +
    /// optional background/border). Origins are relative to `(0, 0)`.
    pub items: Vec<PositionedItem>,
    /// Baseline of the first line, measured from the top of the paragraph.
    pub first_baseline: f32,
    /// Baseline of the last line, measured from the top of the paragraph.
    pub last_baseline: f32,
}

/// Lay out a single paragraph using Parley.
///
/// `text_content` is the flattened text from all inline runs. `style_spans`
/// maps byte ranges to resolved character properties. `available_width` is
/// the maximum line width in points. `display_scale` is the HiDPI scale
/// factor (use `1.0` for layout-only / headless use).
pub fn layout_paragraph(
    resources: &mut FontResources,
    text_content: &str,
    style_spans: &[StyleSpan],
    para_props: &ResolvedParaProps,
    available_width: f32,
    display_scale: f32,
) -> ParagraphLayout {
    if text_content.is_empty() {
        return ParagraphLayout { height: 0.0, width: 0.0, items: vec![], first_baseline: 0.0, last_baseline: 0.0 };
    }

    let mut builder = resources.layout_cx.ranged_builder(
        &mut resources.font_cx,
        text_content,
        display_scale,
        true,
    );

    // Paragraph-level defaults.
    builder.push_default(StyleProperty::Brush(LayoutColor::BLACK));
    builder.push_default(StyleProperty::FontSize(12.0));
    if let Some(lh) = para_props.line_height {
        builder.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(lh)));
    }

    // Per-span styles.
    for span in style_spans {
        let r = span.range.clone();
        builder.push(StyleProperty::FontSize(span.font_size), r.clone());
        builder.push(StyleProperty::Brush(span.color), r.clone());
        if span.bold {
            builder.push(StyleProperty::FontWeight(FontWeight::BOLD), r.clone());
        }
        if span.italic {
            builder.push(StyleProperty::FontStyle(FontStyle::Italic), r.clone());
        }
        if let Some(ref name) = span.font_name {
            builder.push(StyleProperty::FontFamily(FontFamily::named(name.as_str())), r.clone());
        }
        if let Some(lh) = span.line_height {
            builder.push(StyleProperty::LineHeight(LineHeight::FontSizeRelative(lh)), r.clone());
        }
        if span.underline {
            builder.push(StyleProperty::Underline(true), r.clone());
        }
        if span.strikethrough {
            builder.push(StyleProperty::Strikethrough(true), r.clone());
        }
    }

    let mut layout = builder.build(text_content);

    let line_w = (available_width - para_props.indent_start - para_props.indent_end).max(0.0);
    layout.break_all_lines(Some(line_w));
    layout.align(Some(line_w), para_props.alignment, AlignmentOptions::default());

    let total_height = layout.height();
    let total_width = layout.width();
    let first_baseline = layout.lines().next().map(|l| l.metrics().baseline).unwrap_or(0.0);
    let last_baseline = layout.lines().last().map(|l| l.metrics().baseline).unwrap_or(0.0);

    let mut items: Vec<PositionedItem> = Vec::new();

    for line in layout.lines() {
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else { continue };
            let run = glyph_run.run();
            let style = glyph_run.style();
            let run_offset = glyph_run.offset();
            let run_baseline = glyph_run.baseline();

            let font_data = Arc::new(run.font().data.data().to_vec());
            let synthesis = run.synthesis();
            let glyphs: Vec<GlyphEntry> = glyph_run
                .positioned_glyphs()
                .map(|g| GlyphEntry {
                    id: g.id as u16,
                    x: g.x - run_offset,
                    y: g.y - run_baseline,
                    advance: g.advance,
                })
                .collect();

            items.push(PositionedItem::GlyphRun(PositionedGlyphRun {
                origin: LayoutPoint { x: run_offset + para_props.indent_start, y: run_baseline },
                font_data,
                font_index: run.font().index,
                font_size: run.font_size(),
                glyphs,
                color: style.brush.clone(),
                synthesis: GlyphSynthesis { bold: synthesis.embolden(), italic: synthesis.skew().is_some() },
            }));

            // Underline decoration.
            if let Some(deco) = &style.underline {
                let m = run.metrics();
                items.push(PositionedItem::Decoration(PositionedDecoration {
                    x: run_offset + para_props.indent_start,
                    y: run_baseline + deco.offset.unwrap_or(m.underline_offset),
                    width: glyph_run.advance(),
                    thickness: deco.size.unwrap_or(m.underline_size),
                    kind: DecorationKind::Underline,
                    color: deco.brush.clone(),
                }));
            }

            // Strikethrough decoration.
            if let Some(deco) = &style.strikethrough {
                let m = run.metrics();
                items.push(PositionedItem::Decoration(PositionedDecoration {
                    x: run_offset + para_props.indent_start,
                    y: run_baseline + deco.offset.unwrap_or(m.strikethrough_offset),
                    width: glyph_run.advance(),
                    thickness: deco.size.unwrap_or(m.strikethrough_size),
                    kind: DecorationKind::Strikethrough,
                    color: deco.brush.clone(),
                }));
            }
        }
    }

    // Prepend border (below background so it renders on top).
    let has_border = para_props.border_top.is_some()
        || para_props.border_right.is_some()
        || para_props.border_bottom.is_some()
        || para_props.border_left.is_some();
    if has_border {
        let bw = total_width + para_props.indent_start + para_props.indent_end;
        items.insert(0, PositionedItem::BorderRect(PositionedBorderRect {
            rect: LayoutRect::new(0.0, 0.0, bw, total_height),
            top: para_props.border_top,
            right: para_props.border_right,
            bottom: para_props.border_bottom,
            left: para_props.border_left,
        }));
    }

    // Prepend background fill.
    if let Some(bg) = para_props.background_color {
        let bw = total_width + para_props.indent_start + para_props.indent_end;
        items.insert(0, PositionedItem::FilledRect(PositionedRect {
            rect: LayoutRect::new(0.0, 0.0, bw, total_height),
            color: bg,
        }));
    }

    ParagraphLayout { height: total_height, width: total_width, items, first_baseline, last_baseline }
}

#[cfg(test)]
#[path = "para_tests.rs"]
mod tests;
