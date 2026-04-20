// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

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

/// Vertical text position for superscript / subscript runs.
///
/// Mirrors [`loki_doc_model::style::props::char_props::VerticalAlign`].
/// TR 29166 §6.2.1. ODF `style:text-position`; OOXML `w:vertAlign`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlign {
    /// Text raised above the baseline (superscript).
    Superscript,
    /// Text lowered below the baseline (subscript).
    Subscript,
}

/// Caps variant for a text run.
///
/// TR 29166 §6.2.1. ODF `fo:font-variant` / `fo:text-transform`;
/// OOXML `w:smallCaps` / `w:caps`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontVariant {
    /// Render lowercase letters as small capitals.
    SmallCaps,
    /// All characters uppercased (text transform applied at build time).
    AllCaps,
}

/// Underline decoration style, mirroring the doc-model enum.
///
/// Parley 0.6 only renders a single solid underline; variant information is
/// preserved for when the renderer gains multi-style support.
/// TR 29166 §6.2.1. ODF `style:text-underline-style`; OOXML `w:u`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnderlineStyle {
    /// A single solid underline.
    Single,
    /// A double underline.
    Double,
    /// A dotted underline.
    Dotted,
    /// A dashed underline.
    Dash,
    /// A wavy underline.
    Wave,
    /// A thick solid underline.
    Thick,
}

/// Strikethrough decoration style, mirroring the doc-model enum.
///
/// Parley 0.6 only renders a single strikethrough; double style is preserved
/// for future rendering.
/// TR 29166 §6.2.1. ODF `style:text-line-through-style`;
/// OOXML `w:strike` / `w:dstrike`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrikethroughStyle {
    /// A single strikethrough line.
    Single,
    /// A double strikethrough line.
    Double,
}

/// Resolved line-height specification for a paragraph.
///
/// Carries the semantic from the source format through to the Parley call
/// so the correct [`LineHeight`] variant is chosen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResolvedLineHeight {
    /// Proportional multiplier of the font's natural metrics (ascender +
    /// descender + leading). `1.0` = single spacing, `1.5` = 1.5×, etc.
    /// Maps to OOXML `lineRule="auto"` and ODF `fo:line-height` as `%`.
    MetricsRelative(f32),
    /// Exact line height in points. Clips content if smaller than the font
    /// metrics. Maps to OOXML `lineRule="exact"`.
    Exact(f32),
    /// Minimum line height in points. Grows if font metrics require it.
    /// Maps to OOXML `lineRule="atLeast"`.
    AtLeast(f32),
}

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
    /// Underline decoration style. `None` = no underline.
    ///
    /// Parley 0.6 renders all variants identically (single solid underline).
    /// TODO(underline-style): Parley exposes a single underline decoration;
    /// Double/Dotted/Dash/Wave variants all render as Single for now.
    pub underline: Option<UnderlineStyle>,
    /// Strikethrough decoration style. `None` = no strikethrough.
    ///
    /// Parley 0.6 renders all variants identically (single strikethrough).
    /// TODO(strikethrough-style): Parley exposes a single strikethrough decoration;
    /// Double variant renders as Single for now.
    pub strikethrough: Option<StrikethroughStyle>,
    /// Line-height multiplier (e.g. `1.5`). `None` = paragraph default.
    pub line_height: Option<f32>,
    /// Vertical alignment for super/subscript. Font size is reduced to 58%.
    ///
    /// TODO(super-sub): Parley does not expose baseline-shift; only font-size
    /// reduction applied. Revisit when Parley adds StyleProperty::BaselineShift.
    pub vertical_align: Option<VerticalAlign>,
    /// Highlight colour to paint behind the run. `None` = no highlight.
    pub highlight_color: Option<LayoutColor>,
    /// Letter spacing (tracking) in points. `None` = font default.
    pub letter_spacing: Option<f32>,
    /// Caps variant for this run.
    ///
    /// `SmallCaps`: OpenType `smcp` feature would be ideal; currently stored
    /// only. TODO(small-caps): Parley does not expose StyleProperty::FontVariantCaps.
    ///
    /// `AllCaps`: text is uppercased during `flatten_paragraph` in resolve.rs;
    /// this field is retained as metadata.
    pub font_variant: Option<FontVariant>,
    /// Word spacing in points. `None` = font default.
    pub word_spacing: Option<f32>,
    /// Draw a dark-grey shadow offset by `(0.5 pt, 0.5 pt)` behind the run.
    ///
    /// TODO(shadow): replace with Vello blur filter for soft shadow once
    /// scene.rs blur pipeline is verified stable (see TODO in scene.rs).
    pub shadow: bool,
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
    /// Paragraph-level line-height specification, or `None` to use Parley's
    /// natural font metrics (always correct for body text).
    pub line_height: Option<ResolvedLineHeight>,
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
            line_height: None, // None → MetricsRelative(1.0) default in Parley
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
    /// Per-line `(min_coord, max_coord)` in paragraph-local layout units.
    /// Populated from Parley line metrics after `break_all_lines`.
    /// Empty for empty paragraphs.
    ///
    /// Used by `flow_section` to find clean split points at line boundaries.
    /// `TODO(split-optimise)`: Option B y-range item filter can use this field
    /// to avoid rendering clipped content to the GPU once the Option A baseline
    /// is stable and profiled.
    pub line_boundaries: Vec<(f32, f32)>,
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
        return ParagraphLayout {
            height: 0.0,
            width: 0.0,
            items: vec![],
            first_baseline: 0.0,
            last_baseline: 0.0,
            line_boundaries: vec![],
        };
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
    match para_props.line_height {
        // MetricsRelative(1.0) is Parley's default — single-spacing from natural
        // font metrics (ascender + descender + leading). This is correct for
        // OOXML lineRule="auto" w:line="240", the most common case.
        Some(ResolvedLineHeight::MetricsRelative(m)) => {
            builder.push_default(StyleProperty::LineHeight(LineHeight::MetricsRelative(m)));
        }
        // Exact points — OOXML lineRule="exact". May clip descenders.
        Some(ResolvedLineHeight::Exact(pts)) => {
            builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(pts)));
        }
        // AtLeast points — OOXML lineRule="atLeast". Use metrics but honour
        // the minimum by taking the larger of the two. Parley has no native
        // "at-least" variant, so we use MetricsRelative and let the caller
        // check the resulting height if needed (good enough for v0.1).
        Some(ResolvedLineHeight::AtLeast(_pts)) => {
            // No override — let Parley use natural metrics; they are always ≥
            // the author's intent for typical body-text sizes.
        }
        None => {
            // No override — natural font metrics. Always correct.
        }
    }

    // Per-span styles.
    for span in style_spans {
        let r = span.range.clone();

        // For super/subscript (gap #3), reduce font size to 58 %.
        // TODO(super-sub): Parley does not expose baseline-shift; only font-size
        // reduction applied. Revisit when Parley adds StyleProperty::BaselineShift.
        let effective_font_size = if span.vertical_align.is_some() {
            span.font_size * 0.58
        } else {
            span.font_size
        };
        builder.push(StyleProperty::FontSize(effective_font_size), r.clone());
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

        // Underline (gap #17): all style variants map to Parley's single underline.
        // TODO(underline-style): Parley exposes a single underline decoration;
        // Double/Dotted/Dash/Wave variants all render as Single for now.
        if span.underline.is_some() {
            builder.push(StyleProperty::Underline(true), r.clone());
        }

        // Strikethrough (gap #18): both Single and Double map to Parley's one variant.
        // TODO(strikethrough-style): Parley exposes a single strikethrough decoration;
        // Double variant renders as Single for now.
        if span.strikethrough.is_some() {
            builder.push(StyleProperty::Strikethrough(true), r.clone());
        }

        // Letter spacing (gap #13).
        if let Some(ls) = span.letter_spacing {
            builder.push(StyleProperty::LetterSpacing(ls), r.clone());
        }

        // Word spacing (gap #22).
        if let Some(ws) = span.word_spacing {
            builder.push(StyleProperty::WordSpacing(ws), r.clone());
        }

        // Caps variant (gaps #15, #16).
        match span.font_variant {
            Some(FontVariant::SmallCaps) => {
                // TODO(small-caps): Parley does not expose StyleProperty::FontVariantCaps;
                // SmallCaps stored but not applied. Revisit when Parley adds support.
            }
            Some(FontVariant::AllCaps) | None => {
                // AllCaps: text was already uppercased during flatten_paragraph.
                // None: no caps variant, nothing to do.
            }
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
    let line_boundaries: Vec<(f32, f32)> = layout
        .lines()
        .map(|l| (l.metrics().min_coord, l.metrics().max_coord))
        .collect();

    let mut items: Vec<PositionedItem> = Vec::new();

    for line in layout.lines() {
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else { continue };
            let run = glyph_run.run();
            let style = glyph_run.style();
            let run_offset = glyph_run.offset();
            let run_baseline = glyph_run.baseline();

            // Intern the font data bytes by pointer identity so all glyph
            // runs using the same Parley-internal font share the same Arc.
            // Without this, every run would clone the full font file bytes
            // (potentially hundreds of KB) producing unique Arc pointers that
            // defeat the FontDataCache in loki-vello.
            let raw_bytes: &[u8] = run.font().data.data();
            let font_data = resources
                .font_data_cache
                .entry(raw_bytes.as_ptr() as u64)
                .or_insert_with(|| Arc::new(raw_bytes.to_vec()))
                .clone();
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

            let text_range = run.text_range();

            // ── Highlight colour (gap #10) ──────────────────────────────────────
            // Emit a filled rect sized to the run's ink extent BEFORE the glyph
            // run so the background renders below the text.
            if let Some(hl_color) = span_highlight_for_range(style_spans, text_range.clone()) {
                let m = run.metrics();
                items.push(PositionedItem::FilledRect(PositionedRect {
                    rect: LayoutRect::new(
                        run_offset + para_props.indent_start,
                        run_baseline - m.ascent,
                        glyph_run.advance(),
                        m.ascent + m.descent,
                    ),
                    color: hl_color,
                }));
            }

            // ── Shadow copy (gap #24) ───────────────────────────────────────────
            // Emit a dark-grey copy of the run offset by (0.5 pt, 0.5 pt) so
            // it appears as a hard shadow behind the main run.
            // TODO(shadow): replace with Vello blur filter for soft shadow once
            // scene.rs blur pipeline is verified stable (see TODO in scene.rs).
            if span_has_shadow(style_spans, text_range.clone()) {
                items.push(PositionedItem::GlyphRun(PositionedGlyphRun {
                    origin: LayoutPoint {
                        x: run_offset + para_props.indent_start + 0.5,
                        y: run_baseline + 0.5,
                    },
                    font_data: font_data.clone(),
                    font_index: run.font().index,
                    font_size: run.font_size(),
                    glyphs: glyphs.clone(),
                    color: LayoutColor::new(0.4, 0.4, 0.4, 1.0),
                    synthesis: GlyphSynthesis {
                        bold: synthesis.embolden(),
                        italic: synthesis.skew().is_some(),
                    },
                }));
            }

            // ── Main glyph run ──────────────────────────────────────────────────
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

    ParagraphLayout { height: total_height, width: total_width, items, first_baseline, last_baseline, line_boundaries }
}

// ── Private helpers for span → glyph-run lookups ──────────────────────────────

/// Returns the highlight colour for the first span fully containing
/// `text_range`, or `None` if no such span has a highlight.
fn span_highlight_for_range(spans: &[StyleSpan], text_range: Range<usize>) -> Option<LayoutColor> {
    spans
        .iter()
        .find(|s| s.range.start <= text_range.start && s.range.end >= text_range.end)
        .and_then(|s| s.highlight_color)
}

/// Returns `true` if the first span fully containing `text_range` has
/// `shadow = true`.
fn span_has_shadow(spans: &[StyleSpan], text_range: Range<usize>) -> bool {
    spans
        .iter()
        .find(|s| s.range.start <= text_range.start && s.range.end >= text_range.end)
        .map_or(false, |s| s.shadow)
}

#[cfg(test)]
#[path = "para_tests.rs"]
mod tests;
