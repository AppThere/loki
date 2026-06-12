// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Public type definitions for paragraph layout.
//!
//! Contains all enums and structs that form the input and output API of the
//! paragraph layout subsystem (except [`ParagraphLayout`] itself, which lives
//! in [`super::layout_result`]).

use std::ops::Range;

use loki_doc_model::style::list_style::ListId;
use parley::Alignment;

use crate::color::LayoutColor;
use crate::geometry::LayoutInsets;
use crate::items::BorderEdge;

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
