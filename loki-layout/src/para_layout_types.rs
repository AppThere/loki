// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph-level property and layout-result types (split from `para.rs` for
//! the 300-line ceiling): [`ResolvedParaProps`] (+ `WrapBand`), and the
//! [`ParagraphLayout`] result with its hit-test types (`Affinity`,
//! `HitTestResult`, `CursorRect`). The character-level style value types live
//! in `para_types.rs`; the read-only query methods on `ParagraphLayout` live
//! in `para_query.rs`. All are re-exported from `para.rs`.

use std::sync::Arc;

use parley::Alignment;

use super::{ResolvedLineHeight, ResolvedListMarker, ResolvedTabStop};
use crate::color::LayoutColor;
use crate::geometry::LayoutInsets;
use crate::items::{BorderEdge, PositionedItem};

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
    /// natural font metrics.
    pub line_height: Option<ResolvedLineHeight>,
    /// Optional paragraph background fill.
    pub background_color: Option<LayoutColor>,
    /// Optional `w:shd` line/cross hatch shading. When set, the renderer draws
    /// the hatch lines instead of a flat `background_color` fill.
    pub background_hatch: Option<loki_doc_model::style::props::shading::ShadingPattern>,
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
    /// Orphan control: min of the paragraph's *first* lines allowed alone at a
    /// page bottom before a split (`2` = Word default, `0` off; see `flow_para`).
    pub orphan_min: u8,
    /// Widow control: min of the paragraph's *last* lines allowed alone atop the
    /// next page (`2` = Word default, `0` off).
    pub widow_min: u8,
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
    /// Explicit tab stops, sorted ascending by position. Empty = fall back to
    /// the `default_tab_stop` grid. Gap #7.
    pub tab_stops: Vec<ResolvedTabStop>,
    /// Tab-stop grid interval (points) used once `tab_stops` is exhausted;
    /// `36.0` (½ inch) unless `DocumentSettings::default_tab_stop_pt` overrides.
    pub default_tab_stop: f32,
    /// Break an over-long word at any character (`overflow-wrap: anywhere`);
    /// set for table-cell content so it wraps to the fixed column width (Word).
    pub break_long_words: bool,
    /// Dropped-initial spec, or `None`. When set (and the paragraph qualifies —
    /// see [`layout_paragraph`]), the leading character(s) span `lines` rows with
    /// body text beside them. OOXML `w:framePr`/`w:dropCap`, ODF `style:drop-cap`.
    pub drop_cap: Option<loki_doc_model::style::props::drop_cap::DropCap>,
    /// A leading side band the first lines must clear (a floating image the
    /// text wraps around). Set by the flow engine; `None` for normal paragraphs.
    pub wrap_band: Option<WrapBand>,
    /// Author colour of a tracked paragraph-mark (¶) deletion on this block —
    /// paints a struck end-of-paragraph marker (Review, 4a.2). `None` = none.
    pub para_mark_deleted_color: Option<LayoutColor>,
}

/// A side band (a floating object) the first lines of a paragraph wrap around.
///
/// Set on [`ResolvedParaProps::wrap_band`] by the flow engine; consumed by the
/// banded layout path. Drop caps build their band internally instead.
#[derive(Debug, Clone, Copy)]
pub struct WrapBand {
    /// Horizontal width (points) to clear, including the gap to the text.
    pub inset: f32,
    /// Vertical extent (points) the band covers from the paragraph top.
    pub cover_height: f32,
    /// `true` when the object is on the left (text shifts right); `false` when
    /// on the right (text narrows but does not shift).
    pub shift_text: bool,
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
            background_hatch: None,
            border_top: None,
            border_bottom: None,
            border_left: None,
            border_right: None,
            padding: LayoutInsets::default(),
            keep_together: false,
            keep_with_next: false,
            // Word/LibreOffice enable widow/orphan control by default (2 lines).
            orphan_min: 2,
            widow_min: 2,
            page_break_before: false,
            page_break_after: false,
            indent_hanging: 0.0,
            list_marker: None,
            tab_stops: Vec::new(),
            default_tab_stop: 36.0,
            break_long_words: false,
            drop_cap: None,
            wrap_band: None,
            para_mark_deleted_color: None,
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
    /// The Option-B y-range item filter (`items_in_y_range`, feature 6.3)
    /// complements it: each split fragment carries only the items near its
    /// own y-range instead of a full copy of the paragraph's items.
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
    /// Number of leading lines shifted right by [`Self::drop_shift`] to clear a
    /// dropped initial (or a float band in the editor fallback). `0` = none.
    ///
    /// Editing geometry ([`Self::line_indent`]) adds the shift to these lines so
    /// the caret, hit-test, and selection coordinates line up with the rendered
    /// glyphs (the Parley layout is built in an un-shifted coordinate space).
    pub drop_lines: usize,
    /// Horizontal shift in points applied to the first [`Self::drop_lines`]
    /// lines. `0.0` = none.
    pub drop_shift: f32,
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
