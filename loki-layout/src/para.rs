// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph-level layout using Parley.
//!
//! [`layout_paragraph`] takes a flattened text string with ranged
//! [`StyleSpan`]s and paragraph properties, runs Parley shaping and
//! line-breaking, then converts the result into renderer-agnostic
//! [`PositionedItem`]s whose origins are relative to the paragraph's
//! own `(0, 0)` top-left corner.

use std::ops::Range;
use std::sync::Arc;

use loki_doc_model::style::list_style::{
    BulletChar, ListId, ListLevel, ListLevelKind, NumberingScheme,
};
use parley::{
    Alignment, AlignmentOptions, Cursor, FontFamily, FontStyle, FontWeight, InlineBox,
    InlineBoxKind, LineHeight, PositionedLayoutItem, RangedBuilder, Selection, StyleProperty,
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
    /// Bold weight (legacy boolean; retained for synthesis fallback). Prefer
    /// [`Self::weight`] for the effective numeric weight.
    pub bold: bool,
    /// Effective numeric font weight (1–1000; 400 = Regular, 700 = Bold). This
    /// is the value pushed to Parley, so it supersedes `bold` when set from a
    /// `font_weight` style.
    pub weight: u16,
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
    /// MathML markup for an [`Inline::Math`][loki_doc_model::content::inline::Inline::Math]
    /// placeholder. When `Some`, this span has an empty `range` marking the
    /// insertion point of an equation; [`layout_paragraph`] typesets it (see
    /// [`crate::math`]) and places it inline via a Parley inline box. All other
    /// span fields supply the base font size / colour for the math.
    pub math: Option<std::sync::Arc<str>>,
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
    /// Original to cleaned byte index mappings.
    pub orig_to_clean: Vec<usize>,
    /// Cleaned to original byte index mappings.
    pub clean_to_orig: Vec<usize>,
    /// Paragraph start (left) indent in points, applied to drawn glyphs.
    ///
    /// Retained so cursor / hit-test / selection geometry can include the same
    /// horizontal offset the glyph runs use (the Parley layout itself is built
    /// in an un-indented coordinate space). See [`Self::line_indent`].
    pub indent_start: f32,
    /// Hanging indent in points (first line starts this far left of
    /// `indent_start`). `0.0` = no hanging.
    pub indent_hanging: f32,
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
            .field(
                "parley_layout",
                &self.parley_layout.as_ref().map(|_| "<Layout>"),
            )
            .field("orig_to_clean", &self.orig_to_clean)
            .field("clean_to_orig", &self.clean_to_orig)
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
        // Derive the line index from `line_boundaries`: find the first line
        // whose `max_coord` is strictly above the hit y, or clamp to the last line.
        let line_index = self
            .line_boundaries
            .iter()
            .position(|&(_, max_y)| y < max_y)
            .unwrap_or_else(|| self.line_boundaries.len().saturating_sub(1));
        // Glyphs are drawn shifted right by the line's indent, but the Parley
        // layout is un-indented — remove the indent before hit-testing so a
        // click on the visible text maps to the right offset.
        let local_x = x - self.line_indent(line_index);
        let cursor = Cursor::from_point(layout, local_x, y);
        let byte_offset = cursor.index();
        let mapped_offset = self
            .clean_to_orig
            .get(byte_offset)
            .copied()
            .unwrap_or_else(|| self.clean_to_orig.last().copied().unwrap_or(0));
        let affinity = match cursor.affinity() {
            parley::Affinity::Upstream => Affinity::Upstream,
            parley::Affinity::Downstream => Affinity::Downstream,
        };
        Some(HitTestResult {
            byte_offset: mapped_offset,
            affinity,
            line_index,
        })
    }

    /// Returns the byte offset at the end of the visual line that contains
    /// `byte_offset`, optionally trimming a trailing hard-break character.
    ///
    /// `text` is the same UTF-8 string used to build this layout; it is needed
    /// only to check for a trailing `\n` byte that Parley may include in the
    /// line's [`text_range`].  For soft-wrapped lines the range end IS the
    /// correct cursor position (the character sits at the wrap boundary on the
    /// current line with upstream affinity).  For hard-break lines the `\n` is
    /// excluded so the cursor stays after the last visible glyph.
    ///
    /// Returns `None` when hit-test data is not available (read-only mode) or
    /// when the paragraph has no lines.
    pub fn line_end_offset(&self, byte_offset: usize, text: &str) -> Option<usize> {
        let layout = self.parley_layout.as_ref()?;
        let clean_offset = self
            .orig_to_clean
            .get(byte_offset)
            .copied()
            .unwrap_or_else(|| self.orig_to_clean.last().copied().unwrap_or(0));
        // Find the line whose text range contains clean_offset, or fall back to
        // the last line (handles cursor positioned at text.len()).
        let line = layout
            .lines()
            .find(|l| {
                let r = l.text_range();
                r.start <= clean_offset && clean_offset < r.end
            })
            .or_else(|| layout.lines().last())?;

        let range = line.text_range();
        let end = range.end;

        let mapped_end = self
            .clean_to_orig
            .get(end)
            .copied()
            .unwrap_or_else(|| self.clean_to_orig.last().copied().unwrap_or(0));

        // Trim a trailing '\n' or '\r\n' so End lands before the newline byte, not after.
        // In loki-text, paragraph breaks are modelled as separate blocks, so
        // '\n' inside a block's text is unusual — this guard handles edge cases.
        let mut trimmed = mapped_end;
        if trimmed > 0 && text.as_bytes().get(trimmed - 1).copied() == Some(b'\n') {
            trimmed -= 1;
        }
        if trimmed > 0 && text.as_bytes().get(trimmed - 1).copied() == Some(b'\r') {
            trimmed -= 1;
        }

        Some(trimmed)
    }

    /// Returns the visual rectangle for a cursor at the given byte offset in
    /// paragraph-local coordinates.
    ///
    /// Returns `None` when hit-test data is not available (read-only mode).
    /// When `byte_offset` is out of range it is clamped to the nearest valid
    /// position by Parley.
    pub fn cursor_rect(&self, byte_offset: usize) -> Option<CursorRect> {
        let layout = self.parley_layout.as_deref()?;
        let clean_offset = self
            .orig_to_clean
            .get(byte_offset)
            .copied()
            .unwrap_or_else(|| self.orig_to_clean.last().copied().unwrap_or(0));
        let cursor = Cursor::from_byte_index(layout, clean_offset, parley::Affinity::Downstream);
        // width=1.0 requests a 1-point wide caret geometry.
        let bb = cursor.geometry(layout, 1.0);
        let y = bb.y0 as f32;
        let height = (bb.y1 - bb.y0) as f32;
        // Add the line's indent so the caret sits with the drawn glyphs (the
        // Parley layout is built in an un-indented coordinate space). The line is
        // located from the caret's vertical centre, matching `hit_test_point`.
        let probe_y = y + height * 0.5;
        let line_index = self
            .line_boundaries
            .iter()
            .position(|&(_, max_y)| probe_y < max_y)
            .unwrap_or_else(|| self.line_boundaries.len().saturating_sub(1));
        Some(CursorRect {
            x: bb.x0 as f32 + self.line_indent(line_index),
            y,
            height,
        })
    }

    /// Selection highlight rectangles (paragraph-local layout points) covering
    /// the byte range `[start, end)`, one or more per visual line.  Empty when
    /// the range is collapsed, out of editing mode, or has no glyphs.
    ///
    /// Byte offsets are clamped into range. Used for selection painting in both
    /// view modes via [`crate::ContinuousLayout::selection_rects`].
    pub fn selection_rects(&self, start: usize, end: usize) -> Vec<LayoutRect> {
        let Some(layout) = self.parley_layout.as_deref() else {
            return Vec::new();
        };
        let to_clean = |b: usize| {
            self.orig_to_clean
                .get(b)
                .copied()
                .unwrap_or_else(|| self.orig_to_clean.last().copied().unwrap_or(0))
        };
        let (lo, hi) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        let anchor = Cursor::from_byte_index(layout, to_clean(lo), parley::Affinity::Downstream);
        let focus = Cursor::from_byte_index(layout, to_clean(hi), parley::Affinity::Downstream);
        Selection::new(anchor, focus)
            .geometry(layout)
            .into_iter()
            .map(|(bb, line)| {
                LayoutRect::new(
                    bb.x0 as f32 + self.line_indent(line),
                    bb.y0 as f32,
                    (bb.x1 - bb.x0) as f32,
                    (bb.y1 - bb.y0) as f32,
                )
            })
            .collect()
    }

    /// Horizontal indent (points) applied to the drawn glyphs of visual line
    /// `line_index`, matching the `indent_x` used when emitting glyph runs: the
    /// first line of a hanging-indent paragraph starts `indent_hanging` to the
    /// left of `indent_start`. Editing geometry adds this so cursor, hit-test,
    /// and selection coordinates line up with the rendered text.
    fn line_indent(&self, line_index: usize) -> f32 {
        if line_index == 0 && self.indent_hanging > 0.0 {
            self.indent_start - self.indent_hanging
        } else {
            self.indent_start
        }
    }
}

// ── Tab stop helpers (gap #7) ─────────────────────────────────────────────────

// TODO(tab-default): use Document.settings.default_tab_stop_pt once
// DocumentSettings is threaded through layout_document.
/// Default tab stop interval: 0.5 inch = 36 pt = 720 twips (Word default).
const DEFAULT_TAB_INTERVAL: f32 = 36.0;

/// Return the next tab stop position strictly greater than `x`.
///
/// Searches `stops` (sorted ascending) first; falls back to the default
/// 36 pt grid when no explicit stop is defined beyond `x`.
fn next_tab_stop(stops: &[ResolvedTabStop], x: f32, indent_hanging: f32) -> f32 {
    if indent_hanging > 0.0 && x < indent_hanging - 0.5 {
        return indent_hanging;
    }
    if let Some(s) = stops.iter().find(|s| s.position > x + 0.5) {
        s.position
    } else {
        ((x / DEFAULT_TAB_INTERVAL).floor() + 1.0) * DEFAULT_TAB_INTERVAL
    }
}

/// Push paragraph-level defaults and per-span character styles onto `builder`.
///
/// Pushes one Parley inline box per typeset math placeholder, sized to the
/// equation's intrinsic box so the surrounding text flows around it. Ids are
/// offset by [`MATH_ID_BASE`] so the post-layout pass can recognise them.
///
/// The box height is the equation's **ascent** only: Parley aligns an inline
/// box's bottom to the text baseline (counting its whole height as ascent), so
/// reserving just the ascent lands the box top at `baseline − ascent`. Drawing
/// the equation there puts its baseline on the text baseline; the descent then
/// hangs below into the line's descent region, exactly like inline text.
fn push_math_inline_boxes(
    builder: &mut RangedBuilder<'_, LayoutColor>,
    math_boxes: &[(usize, crate::math::MathRender)],
) {
    for (i, (index, render)) in math_boxes.iter().enumerate() {
        builder.push_inline_box(InlineBox {
            id: MATH_ID_BASE + i as u64,
            kind: InlineBoxKind::InFlow,
            index: *index,
            width: render.width,
            height: render.ascent,
        });
    }
}

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
        // Math placeholder spans have an empty range and no text — they are
        // typeset separately and reserved via an inline box, so they push no
        // text styles here.
        if span.math.is_some() {
            continue;
        }
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
        // Push the effective numeric weight. `weight` already folds in `bold`
        // (700 when bold, else 400) plus any explicit `font_weight` style, so a
        // non-default weight is honoured even when the bold flag is unset.
        if span.weight != 400 {
            builder.push(
                StyleProperty::FontWeight(FontWeight::new(span.weight as f32)),
                r.clone(),
            );
        }
        if span.italic {
            builder.push(StyleProperty::FontStyle(FontStyle::Italic), r.clone());
        }
        if let Some(ref name) = span.font_name {
            builder.push(
                StyleProperty::FontFamily(FontFamily::named(name.as_str())),
                r.clone(),
            );
        }
        if let Some(lh) = span.line_height {
            builder.push(
                StyleProperty::LineHeight(LineHeight::FontSizeRelative(lh)),
                r.clone(),
            );
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
fn clean_text_and_spans(
    text: &str,
    spans: &[StyleSpan],
) -> (String, Vec<StyleSpan>, Vec<usize>, Vec<usize>) {
    let mut clean_text = String::with_capacity(text.len());
    let mut orig_to_clean = vec![0; text.len() + 1];
    let mut clean_to_orig = Vec::with_capacity(text.len() + 1);

    let mut orig_idx = 0;
    let mut clean_idx = 0;

    for c in text.chars() {
        let c_len = c.len_utf8();
        let keep = c == '\t' || c == '\n' || (!c.is_control() && c != '\u{feff}');
        if keep {
            for i in 0..c_len {
                orig_to_clean[orig_idx + i] = clean_idx + i;
                clean_to_orig.push(orig_idx + i);
            }
            clean_text.push(c);
            orig_idx += c_len;
            clean_idx += c_len;
        } else {
            for i in 0..c_len {
                orig_to_clean[orig_idx + i] = clean_idx;
            }
            orig_idx += c_len;
        }
    }
    orig_to_clean[orig_idx] = clean_idx;
    clean_to_orig.push(orig_idx);

    let clean_spans = spans
        .iter()
        .map(|span| {
            let mut clean_span = span.clone();
            let start = orig_to_clean
                .get(span.range.start)
                .copied()
                .unwrap_or(clean_idx);
            let end = orig_to_clean
                .get(span.range.end)
                .copied()
                .unwrap_or(clean_idx);
            clean_span.range = start..end;
            clean_span
        })
        .collect();

    (clean_text, clean_spans, orig_to_clean, clean_to_orig)
}

/// Inline-box id base for math placeholders, kept clear of the tab-stop ids
/// (which count up from 0) so the two can coexist in one paragraph.
const MATH_ID_BASE: u64 = 1 << 40;

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
///
/// The result is memoised in `resources.para_cache`: when the same inputs are
/// laid out again (e.g. every paragraph except the edited one, on a keystroke)
/// the cached layout is cloned instead of re-shaped. See
/// [`crate::para_cache`].
pub fn layout_paragraph(
    resources: &mut FontResources,
    text_content: &str,
    style_spans: &[StyleSpan],
    para_props: &ResolvedParaProps,
    available_width: f32,
    display_scale: f32,
    preserve_for_editing: bool,
) -> ParagraphLayout {
    let key = crate::para_cache::para_key(
        text_content,
        style_spans,
        para_props,
        available_width,
        display_scale,
        preserve_for_editing,
    );
    if let Some(hit) = resources.para_cache.get(key) {
        return hit;
    }
    let result = layout_paragraph_uncached(
        resources,
        text_content,
        style_spans,
        para_props,
        available_width,
        display_scale,
        preserve_for_editing,
    );
    resources.para_cache.put(key, result.clone());
    result
}

/// Lays out a single paragraph using Parley, without consulting or populating
/// the shaping cache. [`layout_paragraph`] wraps this with memoisation.
fn layout_paragraph_uncached(
    resources: &mut FontResources,
    text_content: &str,
    style_spans: &[StyleSpan],
    para_props: &ResolvedParaProps,
    available_width: f32,
    display_scale: f32,
    preserve_for_editing: bool,
) -> ParagraphLayout {
    let (mut clean_text, mut clean_spans, orig_to_clean, clean_to_orig) =
        clean_text_and_spans(text_content, style_spans);

    for span in &mut clean_spans {
        if let Some(ref name) = span.font_name {
            span.font_name = Some(resources.resolve_font_name(name));
        }
    }

    // ── Inline math (gap) ─────────────────────────────────────────────────────
    // Typeset each `Inline::Math` placeholder span (empty range, `math: Some`)
    // into its own box. Done before the tab/final passes so its intrinsic size
    // can size a Parley inline box, reserving inline space for the equation.
    let mut math_boxes: Vec<(usize, crate::math::MathRender)> = Vec::new();
    for span in &clean_spans {
        if let Some(mathml) = &span.math {
            let render = crate::math::layout_math(
                resources,
                mathml,
                span.font_size,
                span.color,
                display_scale,
            );
            if render.width > 0.0 {
                math_boxes.push((span.range.start, render));
            }
        }
    }
    // A paragraph that contains only math has empty text; give Parley a single
    // space so it still produces a line to anchor the inline box(es).
    if clean_text.is_empty() && !math_boxes.is_empty() {
        clean_text = " ".to_string();
    }

    if clean_text.is_empty() {
        if !preserve_for_editing {
            return ParagraphLayout {
                height: 0.0,
                width: 0.0,
                items: vec![],
                first_baseline: 0.0,
                last_baseline: 0.0,
                line_boundaries: vec![],
                parley_layout: None,
                orig_to_clean,
                clean_to_orig,
                indent_start: para_props.indent_start,
                indent_hanging: para_props.indent_hanging,
            };
        }
        // Build a phantom single-space layout so cursor_rect can return a
        // properly-sized caret for empty paragraphs.  The space forces Parley
        // to produce one line with the paragraph's resolved font metrics.
        // height/line_boundaries are left at zero so empty paragraphs do not
        // affect vertical flow — they remain un-clickable but navigable.
        let mut builder =
            resources
                .layout_cx
                .ranged_builder(&mut resources.font_cx, " ", display_scale, true);
        push_para_styles(&mut builder, para_props, &[]);
        let mut phantom = builder.build(" ");
        phantom.break_all_lines(Some(available_width));
        let first_baseline = phantom
            .lines()
            .next()
            .map(|l| l.metrics().baseline)
            .unwrap_or(0.0);
        return ParagraphLayout {
            height: 0.0,
            width: 0.0,
            items: vec![],
            first_baseline,
            last_baseline: first_baseline,
            line_boundaries: vec![],
            parley_layout: Some(Arc::new(phantom)),
            orig_to_clean,
            clean_to_orig,
            indent_start: para_props.indent_start,
            indent_hanging: para_props.indent_hanging,
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
    let tab_char_positions: Vec<usize> = clean_text
        .char_indices()
        .filter(|(_, c)| *c == '\t')
        .map(|(i, _)| i)
        .collect();

    let tab_inline_widths: Vec<f32> = if !tab_char_positions.is_empty() {
        let mut probe = resources.layout_cx.ranged_builder(
            &mut resources.font_cx,
            &clean_text,
            display_scale,
            true,
        );
        push_para_styles(&mut probe, para_props, &clean_spans);
        for (idx, &pos) in tab_char_positions.iter().enumerate() {
            probe.push_inline_box(InlineBox {
                id: idx as u64,
                kind: InlineBoxKind::InFlow,
                index: pos,
                width: 0.0,
                height: 0.0,
            });
        }
        push_math_inline_boxes(&mut probe, &math_boxes);
        let mut probe_layout = probe.build(&clean_text);
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
            .map(|&x| {
                (next_tab_stop(&para_props.tab_stops, x, para_props.indent_hanging) - x).max(0.0)
            })
            .collect()
    } else {
        vec![]
    };

    // ── Main (final) layout pass ──────────────────────────────────────────────
    let mut builder = resources.layout_cx.ranged_builder(
        &mut resources.font_cx,
        &clean_text,
        display_scale,
        true,
    );
    push_para_styles(&mut builder, para_props, &clean_spans);
    for (idx, &pos) in tab_char_positions.iter().enumerate() {
        let width = tab_inline_widths.get(idx).copied().unwrap_or(0.0);
        builder.push_inline_box(InlineBox {
            id: idx as u64,
            kind: InlineBoxKind::InFlow,
            index: pos,
            width,
            height: 0.0,
        });
    }
    push_math_inline_boxes(&mut builder, &math_boxes);

    let mut layout = builder.build(&clean_text);
    layout.break_all_lines(Some(line_w));
    layout.align(para_props.alignment, AlignmentOptions::default());

    let total_height = layout.height();
    let total_width = layout.width();
    let first_baseline = layout
        .lines()
        .next()
        .map(|l| l.metrics().baseline)
        .unwrap_or(0.0);
    let last_baseline = layout
        .lines()
        .last()
        .map(|l| l.metrics().baseline)
        .unwrap_or(0.0);
    let line_boundaries: Vec<(f32, f32)> = layout
        .lines()
        .map(|l| (l.metrics().block_min_coord, l.metrics().block_max_coord))
        .collect();

    let mut items: Vec<PositionedItem> = Vec::new();
    let mut line_index: usize = 0;
    // Track the lowest point reached by any inline equation. Its baseline is on
    // the text baseline, so a deep denominator can hang below the line's own
    // descent; we grow the paragraph height to cover it so the next block does
    // not overlap it.
    let mut content_bottom = total_height;

    for line in layout.lines() {
        // Hanging indent: the first line shifts left so the marker is visible to
        // the left of `indent_start`. Subsequent lines use the full `indent_start`.
        let indent_x = if line_index == 0 && para_props.indent_hanging > 0.0 {
            para_props.indent_start - para_props.indent_hanging
        } else {
            para_props.indent_start
        };
        for item in line.items() {
            // Math inline box: emit the typeset equation's draw items, offset to
            // the box's resolved position on the line.
            if let PositionedLayoutItem::InlineBox(pib) = &item {
                if pib.id >= MATH_ID_BASE {
                    let mi = (pib.id - MATH_ID_BASE) as usize;
                    if let Some((_, render)) = math_boxes.get(mi) {
                        for prim in &render.items {
                            let mut prim = prim.clone();
                            prim.translate(pib.x + indent_x, pib.y);
                            items.push(prim);
                        }
                        // The box top is at `pib.y` and its baseline at
                        // `pib.y + ascent`; the descent hangs below that.
                        content_bottom = content_bottom.max(pib.y + render.ascent + render.descent);
                    }
                }
                continue;
            }
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };
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

            let link_url = span_link_url_for_range(&clean_spans, text_range.clone());

            // ── Vertical offset for super/subscript (gap #3) ───────────────────
            // Parley does not expose baseline-shift, so font size is reduced to
            // 58 % in push_para_styles. We manually shift the run origin here so
            // the text actually appears above/below the baseline.
            // Superscript: raise by 35 % of the original (pre-reduction) font size.
            // Subscript:   lower by 20 % of the original font size.
            let va_offset = span_vertical_align_for_range(&clean_spans, text_range.clone())
                .map(|(va, orig_size)| match va {
                    VerticalAlign::Superscript => -orig_size * 0.35,
                    VerticalAlign::Subscript => orig_size * 0.20,
                })
                .unwrap_or(0.0);

            // ── Highlight colour (gap #10) ──────────────────────────────────────
            // Emit a filled rect sized to the run's ink extent BEFORE the glyph
            // run so the background renders below the text.
            if let Some(hl_color) = span_highlight_for_range(&clean_spans, text_range.clone()) {
                let m = run.metrics();
                items.push(PositionedItem::FilledRect(PositionedRect {
                    rect: LayoutRect::new(
                        run_offset + indent_x,
                        run_baseline - m.ascent + va_offset,
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
            if span_has_shadow(&clean_spans, text_range.clone()) {
                items.push(PositionedItem::GlyphRun(PositionedGlyphRun {
                    origin: LayoutPoint {
                        x: run_offset + indent_x + 0.5,
                        y: run_baseline + va_offset + 0.5,
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
                origin: LayoutPoint {
                    x: run_offset + indent_x,
                    y: run_baseline + va_offset,
                },
                font_data,
                font_index: run.font().index,
                font_size: run.font_size(),
                glyphs,
                color: style.brush,
                synthesis: GlyphSynthesis {
                    bold: synthesis.embolden(),
                    italic: synthesis.skew().is_some(),
                },
                link_url,
            }));

            // Underline decoration.
            if let Some(deco) = &style.underline {
                let m = run.metrics();
                // COMPAT(parley-0.6): RunMetrics offsets follow OpenType / skrifa
                // Y-up convention (negative = below baseline). Negate to convert
                // to screen Y-down (positive = below baseline).
                items.push(PositionedItem::Decoration(PositionedDecoration {
                    x: run_offset + indent_x,
                    y: run_baseline - deco.offset.unwrap_or(m.underline_offset),
                    width: glyph_run.advance(),
                    thickness: deco.size.unwrap_or(m.underline_size),
                    kind: DecorationKind::Underline,
                    color: deco.brush,
                }));
            }

            // Strikethrough decoration.
            if let Some(deco) = &style.strikethrough {
                let m = run.metrics();
                // COMPAT(parley-0.6): same Y-up → Y-down negation as underline.
                items.push(PositionedItem::Decoration(PositionedDecoration {
                    x: run_offset + indent_x,
                    y: run_baseline - deco.offset.unwrap_or(m.strikethrough_offset),
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
        items.insert(
            0,
            PositionedItem::BorderRect(PositionedBorderRect {
                rect: LayoutRect::new(0.0, 0.0, bw, total_height),
                top: para_props.border_top,
                right: para_props.border_right,
                bottom: para_props.border_bottom,
                left: para_props.border_left,
            }),
        );
    }

    // Prepend background fill.
    if let Some(bg) = para_props.background_color {
        let bw = total_width + para_props.indent_start + para_props.indent_end;
        items.insert(
            0,
            PositionedItem::FilledRect(PositionedRect {
                rect: LayoutRect::new(0.0, 0.0, bw, total_height),
                color: bg,
            }),
        );
    }

    let parley_layout = if preserve_for_editing {
        Some(Arc::new(layout))
    } else {
        None
    };

    ParagraphLayout {
        // `content_bottom` ≥ `total_height`; it is larger only when an inline
        // equation hangs below the last line (see above).
        height: content_bottom,
        width: total_width,
        items,
        first_baseline,
        last_baseline,
        line_boundaries,
        parley_layout,
        orig_to_clean,
        clean_to_orig,
        indent_start: para_props.indent_start,
        indent_hanging: para_props.indent_hanging,
    }
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
        ListLevelKind::Bullet {
            char: BulletChar::Char(c),
            ..
        } => c.to_string(),
        ListLevelKind::Bullet {
            char: BulletChar::Image,
            ..
        } => {
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
            && d.is_ascii_digit()
            && d != '0'
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
        buf.push(if upper {
            byte.to_ascii_uppercase()
        } else {
            byte
        });
        n /= 26;
    }
    buf.reverse();
    String::from_utf8(buf).unwrap_or_default()
}

/// Convert `n` to a Roman numeral string.
fn roman_numeral(n: u32, upper: bool) -> String {
    const TABLE: &[(u32, &str)] = &[
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
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

/// Returns the vertical alignment and original (pre-reduction) font size for
/// the first span fully containing `text_range`, or `None` if no vertical
/// alignment is set on that span.
fn span_vertical_align_for_range(
    spans: &[StyleSpan],
    text_range: Range<usize>,
) -> Option<(VerticalAlign, f32)> {
    spans
        .iter()
        .find(|s| s.range.start <= text_range.start && s.range.end >= text_range.end)
        .and_then(|s| s.vertical_align.map(|va| (va, s.font_size)))
}

#[cfg(test)]
#[path = "para_tests.rs"]
mod tests;
