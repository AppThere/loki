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

use loki_doc_model::style::list_style::{BulletChar, ListId, ListLevel, ListLevelKind, NumberingScheme};
use parley::{
    Alignment, AlignmentOptions, Cursor, FontFamily, FontStyle, FontWeight, InlineBox, LineHeight,
    PositionedLayoutItem, RangedBuilder, StyleProperty,
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

/// Resolved list membership for a list-item paragraph.
///
/// Carries the minimum data the flow engine needs to look up the [`ListStyle`]
/// in [`StyleCatalog`], advance the per-list counter, and synthesise the
/// marker text. Stored in [`ResolvedParaProps::list_marker`].
///
/// [`ListStyle`]: loki_doc_model::style::list_style::ListStyle
/// [`StyleCatalog`]: loki_doc_model::style::catalog::StyleCatalog
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedListMarker {
    /// Which list this paragraph belongs to.
    pub list_id: ListId,
    /// Zero-based nesting level within the list (0 = outermost).
    pub level: u8,
}

/// A resolved tab stop for paragraph layout.
///
/// Parley 0.8 has no native tab stop API; tab characters are expanded
/// to [`InlineBox`] widths in [`layout_paragraph`] using a two-pass approach.
/// TR 29166 §6.2.2. ECMA-376 §17.3.1.37; ODF §16.29.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedTabStop {
    /// Tab stop position from the content-area start edge, in points.
    pub position: f32,
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
    /// Hyperlink URL if this run belongs to a link inline. `None` otherwise.
    ///
    /// Set by `resolve.rs` `walk_inlines` when recursing into `Inline::Link`
    /// children. Used to render a visual link hint and (eventually) hit-test
    /// regions. TODO(link-click): interactive hit-testing deferred.
    pub link_url: Option<String>,
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
    /// If `true` and layout mode is paginated, force a page break immediately
    /// after this paragraph. Gap #20.
    pub page_break_after: bool,
    /// Hanging indent in points: the first line extends this far to the LEFT of
    /// `indent_start` (where the list marker is placed). `0.0` = no hanging.
    /// OOXML `w:ind w:hanging`; gap #8.
    pub indent_hanging: f32,
    /// List membership for this paragraph. `None` for non-list paragraphs.
    pub list_marker: Option<ResolvedListMarker>,
    /// Explicit tab stops, sorted ascending by position. Empty = use the
    /// default 36 pt (0.5 inch) grid. Gap #7.
    pub tab_stops: Vec<ResolvedTabStop>,
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
            page_break_after: false,
            indent_hanging: 0.0,
            list_marker: None,
            tab_stops: Vec::new(),
        }
    }
}

// ── Hit-testing result types ──────────────────────────────────────────────────

/// Cursor affinity — which side of a character cluster a cursor sits on.
///
/// Mirrors `parley::Affinity` but defined in our public API so callers
/// need not depend on the Parley crate directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Affinity {
    /// The cursor sits on the upstream (trailing) edge of the cluster.
    Upstream,
    /// The cursor sits on the downstream (leading) edge of the cluster.
    Downstream,
}

/// Result of a hit test against a paragraph.
///
/// All positions are in paragraph-local coordinates.
#[derive(Debug, Clone, Copy)]
pub struct HitTestResult {
    /// Byte offset into the paragraph's text content.
    pub byte_offset: usize,
    /// Whether the hit falls on the leading or trailing edge of the glyph cluster.
    pub affinity: Affinity,
    /// Zero-based index of the line containing the hit point.
    pub line_index: usize,
}

/// Visual rectangle for a cursor at a given byte offset.
///
/// All positions are in paragraph-local coordinates (points).
#[derive(Debug, Clone, Copy)]
pub struct CursorRect {
    /// X position of the cursor's left edge in paragraph-local coordinates.
    pub x: f32,
    /// Y position of the cursor's top edge in paragraph-local coordinates.
    pub y: f32,
    /// Cursor height (typically the line height).
    pub height: f32,
}

/// The measured result of laying out one paragraph.
#[derive(Clone)]
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
    /// Parley layout object retained for hit testing and cursor positioning.
    ///
    /// `None` in read-only rendering mode (when `preserve_for_editing` is
    /// `false` on the `layout_paragraph` call). Populated only when the caller
    /// opts in so that long read-only documents pay no memory cost.
    ///
    /// Wrapped in `Arc` so `ParagraphLayout` remains cheaply cloneable when
    /// the editing layer shares layouts across the page editing index.
    pub parley_layout: Option<Arc<parley::Layout<LayoutColor>>>,
}

impl std::fmt::Debug for ParagraphLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParagraphLayout")
            .field("height", &self.height)
            .field("width", &self.width)
            .field("items", &self.items)
            .field("first_baseline", &self.first_baseline)
            .field("last_baseline", &self.last_baseline)
            .field("line_boundaries", &self.line_boundaries)
            .field("parley_layout", &self.parley_layout.as_ref().map(|_| "<Layout>"))
            .finish()
    }
}

impl ParagraphLayout {
    /// Returns the character byte offset closest to the given point in
    /// paragraph-local coordinates.
    ///
    /// Returns `None` when hit-test data is not available, i.e. when the
    /// layout was produced with `preserve_for_editing: false` (read-only mode).
    pub fn hit_test_point(&self, x: f32, y: f32) -> Option<HitTestResult> {
        let layout = self.parley_layout.as_deref()?;
        let cursor = Cursor::from_point(layout, x, y);
        let byte_offset = cursor.index();
        let affinity = match cursor.affinity() {
            parley::Affinity::Upstream => Affinity::Upstream,
            parley::Affinity::Downstream => Affinity::Downstream,
        };
        // Derive the line index from `line_boundaries`: find the first line
        // whose `max_coord` is strictly above the hit y, or clamp to the last line.
        let line_index = self
            .line_boundaries
            .iter()
            .position(|&(_, max_y)| y < max_y)
            .unwrap_or_else(|| self.line_boundaries.len().saturating_sub(1));
        Some(HitTestResult { byte_offset, affinity, line_index })
    }

    /// Returns the visual rectangle for a cursor at the given byte offset in
    /// paragraph-local coordinates.
    ///
    /// Returns `None` when hit-test data is not available (read-only mode).
    /// When `byte_offset` is out of range it is clamped to the nearest valid
    /// position by Parley.
    pub fn cursor_rect(&self, byte_offset: usize) -> Option<CursorRect> {
        let layout = self.parley_layout.as_deref()?;
        let cursor = Cursor::from_byte_index(layout, byte_offset, parley::Affinity::Downstream);
        // width=1.0 requests a 1-point wide caret geometry.
        let bb = cursor.geometry(layout, 1.0);
        let y = bb.y0 as f32;
        let height = (bb.y1 - bb.y0) as f32;
        Some(CursorRect { x: bb.x0 as f32, y, height })
    }
}

// ── Tab stop helpers (gap #7) ─────────────────────────────────────────────────

/// Default tab stop interval: 0.5 inch = 36 pt = 720 twips (Word default).
const DEFAULT_TAB_INTERVAL: f32 = 36.0;

/// Return the next tab stop position strictly greater than `x`.
///
/// Searches `stops` (sorted ascending) first; falls back to the default
/// 36 pt grid when no explicit stop is defined beyond `x`.
fn next_tab_stop(stops: &[ResolvedTabStop], x: f32) -> f32 {
    if let Some(s) = stops.iter().find(|s| s.position > x + 0.5) {
        s.position
    } else {
        ((x / DEFAULT_TAB_INTERVAL).floor() + 1.0) * DEFAULT_TAB_INTERVAL
    }
}

/// Push paragraph-level defaults and per-span character styles onto `builder`.
///
/// Extracted so the same styles can be applied in both the probe pass (pass 1)
/// and the final pass (pass 2) of the two-pass tab stop expansion.
fn push_para_styles(
    builder: &mut RangedBuilder<'_, LayoutColor>,
    para_props: &ResolvedParaProps,
    style_spans: &[StyleSpan],
) {
    builder.push_default(StyleProperty::Brush(LayoutColor::BLACK));
    builder.push_default(StyleProperty::FontSize(12.0));
    match para_props.line_height {
        // MetricsRelative(1.0) is Parley's default — single-spacing from natural
        // font metrics. Correct for OOXML lineRule="auto" w:line="240".
        Some(ResolvedLineHeight::MetricsRelative(m)) => {
            builder.push_default(StyleProperty::LineHeight(LineHeight::MetricsRelative(m)));
        }
        Some(ResolvedLineHeight::Exact(pts)) => {
            builder.push_default(StyleProperty::LineHeight(LineHeight::Absolute(pts)));
        }
        // AtLeast: let Parley use natural metrics (always ≥ author intent for body text).
        // None: natural font metrics — always correct.
        Some(ResolvedLineHeight::AtLeast(_)) | None => {}
    }

    for span in style_spans {
        let r = span.range.clone();
        // For super/subscript (gap #3), reduce font size to 58 %.
        // TODO(super-sub): Parley does not expose baseline-shift.
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
        // Underline (gap #17): all variants map to Parley's single underline.
        if span.underline.is_some() {
            builder.push(StyleProperty::Underline(true), r.clone());
        }
        // Strikethrough (gap #18): both variants map to Parley's one variant.
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
        // Caps variant (gaps #15, #16): SmallCaps stored but not applied (no Parley API).
        // AllCaps: text was already uppercased during flatten_paragraph.
    }
}

/// Lay out a single paragraph using Parley.
///
/// `text_content` is the flattened text from all inline runs. `style_spans`
/// maps byte ranges to resolved character properties. `available_width` is
/// the maximum line width in points. `display_scale` is the HiDPI scale
/// factor (use `1.0` for layout-only / headless use).
///
/// When `preserve_for_editing` is `true`, the Parley `Layout` object is
/// retained in [`ParagraphLayout::parley_layout`] so that subsequent editing
/// sessions can call [`ParagraphLayout::hit_test_point`] and
/// [`ParagraphLayout::cursor_rect`]. In read-only rendering mode pass
/// `false` to avoid the memory cost on large documents.
pub fn layout_paragraph(
    resources: &mut FontResources,
    text_content: &str,
    style_spans: &[StyleSpan],
    para_props: &ResolvedParaProps,
    available_width: f32,
    display_scale: f32,
    preserve_for_editing: bool,
) -> ParagraphLayout {
    if text_content.is_empty() {
        return ParagraphLayout {
            height: 0.0,
            width: 0.0,
            items: vec![],
            first_baseline: 0.0,
            last_baseline: 0.0,
            line_boundaries: vec![],
            parley_layout: None,
        };
    }

    // NOTE(indent-hanging-width): Parley 0.6 does not expose per-line width
    // control. The first line of a hanging-indent paragraph wraps at the same
    // `line_w` as subsequent lines, meaning it gets `indent_hanging` px less
    // space than it should. Fix requires Parley to expose per-line measure.
    // Tracked: fidelity audit gap #8 (partial).
    let line_w = (available_width - para_props.indent_start - para_props.indent_end).max(0.0);

    // ── Tab stop expansion (gap #7) ───────────────────────────────────────────
    // Parley 0.8 has no native tab stop API. Two-pass approach:
    //   Pass 1 (probe): zero-width InlineBoxes at each \t → measure x-positions.
    //   Pass 2 (final): InlineBoxes sized to advance to the next tab stop.
    let tab_char_positions: Vec<usize> = text_content
        .char_indices()
        .filter(|(_, c)| *c == '\t')
        .map(|(i, _)| i)
        .collect();

    let tab_inline_widths: Vec<f32> = if !tab_char_positions.is_empty() {
        let mut probe = resources.layout_cx.ranged_builder(
            &mut resources.font_cx,
            text_content,
            display_scale,
            true,
        );
        push_para_styles(&mut probe, para_props, style_spans);
        for (idx, &pos) in tab_char_positions.iter().enumerate() {
            probe.push_inline_box(InlineBox { id: idx as u64, index: pos, width: 0.0, height: 0.0 });
        }
        let mut probe_layout = probe.build(text_content);
        probe_layout.break_all_lines(Some(line_w));

        let mut x_positions = vec![0.0f32; tab_char_positions.len()];
        for line in probe_layout.lines() {
            for item in line.items() {
                if let PositionedLayoutItem::InlineBox(pib) = item {
                    let idx = pib.id as usize;
                    if idx < x_positions.len() {
                        x_positions[idx] = pib.x;
                    }
                }
            }
        }
        x_positions
            .iter()
            .map(|&x| (next_tab_stop(&para_props.tab_stops, x) - x).max(0.0))
            .collect()
    } else {
        vec![]
    };

    // ── Main (final) layout pass ──────────────────────────────────────────────
    let mut builder = resources.layout_cx.ranged_builder(
        &mut resources.font_cx,
        text_content,
        display_scale,
        true,
    );
    push_para_styles(&mut builder, para_props, style_spans);
    for (idx, &pos) in tab_char_positions.iter().enumerate() {
        let width = tab_inline_widths.get(idx).copied().unwrap_or(0.0);
        builder.push_inline_box(InlineBox { id: idx as u64, index: pos, width, height: 0.0 });
    }

    let mut layout = builder.build(text_content);
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
    let mut line_index: usize = 0;

    for line in layout.lines() {
        // Hanging indent: the first line shifts left so the marker is visible to
        // the left of `indent_start`. Subsequent lines use the full `indent_start`.
        let indent_x = if line_index == 0 && para_props.indent_hanging > 0.0 {
            para_props.indent_start - para_props.indent_hanging
        } else {
            para_props.indent_start
        };
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
            let link_url = span_link_url_for_range(style_spans, text_range.clone());

            // ── Highlight colour (gap #10) ──────────────────────────────────────
            // Emit a filled rect sized to the run's ink extent BEFORE the glyph
            // run so the background renders below the text.
            if let Some(hl_color) = span_highlight_for_range(style_spans, text_range.clone()) {
                let m = run.metrics();
                items.push(PositionedItem::FilledRect(PositionedRect {
                    rect: LayoutRect::new(
                        run_offset + indent_x,
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
                        x: run_offset + indent_x + 0.5,
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
                    link_url: None, // shadows don't carry link metadata
                }));
            }

            // ── Main glyph run ──────────────────────────────────────────────────
            items.push(PositionedItem::GlyphRun(PositionedGlyphRun {
                origin: LayoutPoint { x: run_offset + indent_x, y: run_baseline },
                font_data,
                font_index: run.font().index,
                font_size: run.font_size(),
                glyphs,
                color: style.brush,
                synthesis: GlyphSynthesis { bold: synthesis.embolden(), italic: synthesis.skew().is_some() },
                link_url,
            }));

            // Underline decoration.
            if let Some(deco) = &style.underline {
                let m = run.metrics();
                items.push(PositionedItem::Decoration(PositionedDecoration {
                    x: run_offset + indent_x,
                    y: run_baseline + deco.offset.unwrap_or(m.underline_offset),
                    width: glyph_run.advance(),
                    thickness: deco.size.unwrap_or(m.underline_size),
                    kind: DecorationKind::Underline,
                    color: deco.brush,
                }));
            }

            // Strikethrough decoration.
            if let Some(deco) = &style.strikethrough {
                let m = run.metrics();
                items.push(PositionedItem::Decoration(PositionedDecoration {
                    x: run_offset + indent_x,
                    y: run_baseline + deco.offset.unwrap_or(m.strikethrough_offset),
                    width: glyph_run.advance(),
                    thickness: deco.size.unwrap_or(m.strikethrough_size),
                    kind: DecorationKind::Strikethrough,
                    color: deco.brush,
                }));
            }
        }
        line_index += 1;
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

    let parley_layout = if preserve_for_editing {
        Some(Arc::new(layout))
    } else {
        None
    };

    ParagraphLayout { height: total_height, width: total_width, items, first_baseline, last_baseline, line_boundaries, parley_layout }
}

// ── List marker synthesis ─────────────────────────────────────────────────────

/// Produce the display string for a list marker at `level` in `list_levels`.
///
/// Handles bullet characters, all six [`NumberingScheme`] variants, and
/// multi-level `%N`-style format strings (OOXML `w:lvlText`, ODF
/// `text:num-format`). Picture bullets fall back to `"•"`.
///
/// # Arguments
/// * `list_levels` – all level definitions for the list (from `ListStyle.levels`)
/// * `level`       – the zero-based level being rendered
/// * `counters`    – current per-level counter array (all 9 levels)
///
/// Returns an empty string for `ListLevelKind::None`.
pub fn format_list_marker(list_levels: &[ListLevel], level: u8, counters: &[u32; 9]) -> String {
    let Some(level_def) = list_levels.get(level as usize) else {
        return String::new();
    };
    match &level_def.kind {
        ListLevelKind::Bullet { char: BulletChar::Char(c), .. } => c.to_string(),
        ListLevelKind::Bullet { char: BulletChar::Image, .. } => {
            // TODO(list-picture-bullet): picture bullets not yet supported; render as •
            "•".to_string()
        }
        ListLevelKind::Numbered { format, .. } => {
            format_numbered_label(list_levels, format, counters)
        }
        ListLevelKind::None => String::new(),
        // Non-exhaustive guard.
        _ => String::new(),
    }
}

/// Expand a `w:lvlText`-style format string, replacing `%N` tokens with
/// the counter at 0-based level N-1 formatted by that level's scheme.
fn format_numbered_label(list_levels: &[ListLevel], format: &str, counters: &[u32; 9]) -> String {
    let mut result = String::with_capacity(format.len() + 4);
    let mut chars = format.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%'
            && let Some(&d) = chars.peek()
            && d.is_ascii_digit() && d != '0'
        {
            chars.next();
            let level_idx = (d as u8 - b'1') as usize; // 1-based → 0-based
            let counter = counters.get(level_idx).copied().unwrap_or(1);
            let scheme = list_levels
                .get(level_idx)
                .map(|l| match &l.kind {
                    ListLevelKind::Numbered { scheme, .. } => *scheme,
                    _ => NumberingScheme::Decimal,
                })
                .unwrap_or(NumberingScheme::Decimal);
            result.push_str(&format_counter(counter, scheme));
            continue;
        }
        result.push(c);
    }
    result
}

/// Format a single counter value according to its numbering scheme.
fn format_counter(n: u32, scheme: NumberingScheme) -> String {
    match scheme {
        NumberingScheme::Decimal => n.to_string(),
        NumberingScheme::LowerAlpha => alpha_label(n, false),
        NumberingScheme::UpperAlpha => alpha_label(n, true),
        NumberingScheme::LowerRoman => roman_numeral(n, false),
        NumberingScheme::UpperRoman => roman_numeral(n, true),
        NumberingScheme::Ordinal => format!("{}{}", n, ordinal_suffix(n)),
        NumberingScheme::None => String::new(),
        _ => n.to_string(), // non-exhaustive fallback
    }
}

/// Convert `n` to an alphabetic label: 1→a, 2→b, …, 26→z, 27→aa, 28→ab, …
fn alpha_label(mut n: u32, upper: bool) -> String {
    let mut buf = Vec::new();
    while n > 0 {
        n -= 1;
        let byte = b'a' + (n % 26) as u8;
        buf.push(if upper { byte.to_ascii_uppercase() } else { byte });
        n /= 26;
    }
    buf.reverse();
    String::from_utf8(buf).unwrap_or_default()
}

/// Convert `n` to a Roman numeral string.
fn roman_numeral(n: u32, upper: bool) -> String {
    const TABLE: &[(u32, &str)] = &[
        (1000, "m"), (900, "cm"), (500, "d"), (400, "cd"),
        (100,  "c"), (90,  "xc"), (50,  "l"), (40,  "xl"),
        (10,   "x"), (9,   "ix"), (5,   "v"), (4,   "iv"), (1, "i"),
    ];
    let mut n = n;
    let mut s = String::new();
    for &(val, sym) in TABLE {
        while n >= val {
            s.push_str(sym);
            n -= val;
        }
    }
    if upper { s.to_uppercase() } else { s }
}

/// Return the English ordinal suffix for `n` (1st, 2nd, 3rd, …, 11th, …).
fn ordinal_suffix(n: u32) -> &'static str {
    match n % 100 {
        11..=13 => "th",
        _ => match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    }
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

/// Returns the link URL for the first span fully containing `text_range`,
/// or `None` if no span in that range carries a link URL.
fn span_link_url_for_range(spans: &[StyleSpan], text_range: Range<usize>) -> Option<String> {
    spans
        .iter()
        .find(|s| s.range.start <= text_range.start && s.range.end >= text_range.end)
        .and_then(|s| s.link_url.clone())
}

/// Returns `true` if the first span fully containing `text_range` has
/// `shadow = true`.
fn span_has_shadow(spans: &[StyleSpan], text_range: Range<usize>) -> bool {
    spans
        .iter()
        .find(|s| s.range.start <= text_range.start && s.range.end >= text_range.end)
        .is_some_and(|s| s.shadow)
}

#[cfg(test)]
#[path = "para_tests.rs"]
mod tests;
