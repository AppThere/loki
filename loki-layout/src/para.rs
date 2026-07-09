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

use loki_doc_model::style::list_style::ListId;
use loki_doc_model::style::props::tab_stop::{TabAlignment, TabLeader};
use parley::{
    Alignment, AlignmentOptions, FontFamily, FontFeatures, FontStyle, FontWeight, InlineBox,
    InlineBoxKind, LineHeight, OverflowWrap, PositionedLayoutItem, RangedBuilder, StyleProperty,
};

use crate::color::LayoutColor;
use crate::font::FontResources;
use crate::geometry::{LayoutInsets, LayoutRect};
use crate::items::{BorderEdge, PositionedBorderRect, PositionedItem, PositionedRect};

#[path = "para_query.rs"]
mod query;
#[path = "para_tabs.rs"]
mod tabs;
#[path = "para_underlays.rs"]
mod underlays;

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
    /// How text following the tab is aligned relative to [`Self::position`].
    pub alignment: TabAlignment,
    /// Leader character drawn across the tab gap (dots/dashes/…), if any.
    pub leader: TabLeader,
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
    /// Vertical alignment for super/subscript. Font size is reduced to 58% and
    /// the run is shifted via a manual `va_offset` in `para_emit` (plus a
    /// per-glyph `baseline_shift` for `w:position`). TODO(super-sub): the shift is
    /// manual only because Parley lacks a native `StyleProperty::BaselineShift`.
    pub vertical_align: Option<VerticalAlign>,
    /// Highlight colour to paint behind the run. `None` = no highlight.
    pub highlight_color: Option<LayoutColor>,
    /// Letter spacing (tracking) in points. `None` = font default.
    pub letter_spacing: Option<f32>,
    /// Caps variant for this run, retained as metadata.
    ///
    /// Both variants are synthesized during `flatten_paragraph` (resolve.rs),
    /// since Parley exposes no `StyleProperty::FontVariantCaps`:
    /// - `AllCaps`: the text is uppercased.
    /// - `SmallCaps`: the text is uppercased and originally-lowercase letters are
    ///   split into their own spans at a reduced font size (the small-cap look).
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
    /// Horizontal text scale as a fraction (`1.0` = 100 %; `1.5` = 150 % wide).
    /// `None` = no scaling. ODF `style:text-scale`; OOXML `w:w`.
    ///
    /// Applied geometrically to glyph advances and positions at emit time
    /// ([`crate::para_emit::emit_glyph_run`]). COMPAT(parley-0.6): Parley has no
    /// geometric horizontal-scale style, so line-breaking still measures the
    /// unscaled run; following runs on the same line are shifted by the extra
    /// width so they do not overlap, but a scaled run may extend past the right
    /// margin where Word would have wrapped earlier.
    pub scale: Option<f32>,

    /// Apply GPOS pair kerning to this run (gap #23). `Some(true)` = kern;
    /// anything else = off, matching the reference apps' defaults (Word's
    /// `w:kern` threshold defaults to 0 = off; LibreOffice treats an ODT
    /// without `style:letter-kerning` as off). The shaper (harfrust) defaults
    /// kerning ON, so the off case is an explicit feature disable.
    pub kerning: Option<bool>,

    /// Manual baseline shift (text rise) in points; positive raises the glyphs
    /// above the baseline, negative lowers them. `None` = on the baseline.
    /// OOXML `w:position`; ODF `style:text-position`.
    ///
    /// Unlike [`vertical_align`] (super/subscript, which also shrinks the font),
    /// this keeps the font size and is applied per glyph at emit time
    /// ([`crate::para_emit::emit_glyph_run`]) — so it survives Parley coalescing
    /// adjacent runs that differ only in their rise.
    ///
    /// [`vertical_align`]: Self::vertical_align
    pub baseline_shift: Option<f32>,
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
    /// Break an over-long word that does not fit the available width by
    /// allowing a break at any character (CSS `overflow-wrap: anywhere`).
    /// Set for table-cell content so a long unbreakable word wraps to the
    /// fixed column width (matching Word) instead of overflowing into the
    /// neighbouring cell. Normal body paragraphs leave this `false`.
    pub break_long_words: bool,
    /// Dropped-initial specification, or `None`. When set (and the paragraph
    /// qualifies — see [`layout_paragraph`]), the leading character(s) are
    /// enlarged to span `lines` text rows with the body text flowing beside
    /// them. Imported from OOXML `w:framePr`/`w:dropCap` and ODF
    /// `style:drop-cap`.
    pub drop_cap: Option<loki_doc_model::style::props::drop_cap::DropCap>,
    /// A leading side band the first lines of this paragraph must clear (a
    /// floating image the text wraps around). Set by the flow engine; the
    /// banded layout path narrows the lines beside it and reflows the rest at
    /// full width. `None` for normal paragraphs.
    pub wrap_band: Option<WrapBand>,
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
            break_long_words: false,
            drop_cap: None,
            wrap_band: None,
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
pub(crate) fn push_para_styles(
    builder: &mut RangedBuilder<'_, LayoutColor>,
    para_props: &ResolvedParaProps,
    style_spans: &[StyleSpan],
) {
    builder.push_default(StyleProperty::Brush(LayoutColor::BLACK));
    builder.push_default(StyleProperty::FontSize(12.0));
    // Table cells break over-long words to the column width (CSS
    // `overflow-wrap: anywhere`); body paragraphs keep words intact.
    if para_props.break_long_words {
        builder.push_default(StyleProperty::OverflowWrap(OverflowWrap::Anywhere));
    }
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
        // For super/subscript (gap #3), reduce font size to 58 %. The shift is
        // applied in `para_emit` (`va_offset`) — TODO(super-sub): no native API.
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
        // Kerning (gap #23): the reference apps default pair kerning OFF
        // (see StyleSpan::kerning); harfrust defaults it ON — disable unless
        // the document explicitly enables it.
        if span.kerning != Some(true) {
            builder.push(
                StyleProperty::FontFeatures(FontFeatures::from("\"kern\" 0")),
                r.clone(),
            );
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

/// Probe-only inline-box id base for decimal-separator markers (one per tab),
/// used to measure where the first `.` after a tab sits for decimal alignment.
const DEC_ID_BASE: u64 = 1 << 20;

/// Probe-only inline-box id for the end-of-text sentinel, used to measure the
/// trailing edge of the content following the last tab.
const END_ID: u64 = 1 << 30;

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
    layout_paragraph_spelled(
        resources,
        text_content,
        style_spans,
        para_props,
        available_width,
        display_scale,
        preserve_for_editing,
        None,
    )
}

/// [`layout_paragraph`] with an optional spell checker.
///
/// When `spell` is `Some`, misspelled words emit [`DecorationKind::Spelling`]
/// squiggles. The checker's `generation` folds into the cache key so cached
/// layouts are reused only while the dictionary/word-lists are unchanged.
// The sibling `layout_paragraph` already sits at the 7-arg limit; the optional
// spell checker is one more positional input on the same hot path. Bundling
// them into a struct would obscure the call sites for no benefit.
#[allow(clippy::too_many_arguments)]
pub(crate) fn layout_paragraph_spelled(
    resources: &mut FontResources,
    text_content: &str,
    style_spans: &[StyleSpan],
    para_props: &ResolvedParaProps,
    available_width: f32,
    display_scale: f32,
    preserve_for_editing: bool,
    spell: Option<&crate::SpellState>,
) -> ParagraphLayout {
    let spell_generation = spell.map_or(0, |s| s.generation);
    let key = crate::para_cache::para_key(
        text_content,
        style_spans,
        para_props,
        available_width,
        display_scale,
        preserve_for_editing,
        spell_generation,
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
        spell,
    );
    resources.para_cache.put(key, result.clone());
    result
}

/// Prepends the paragraph's border and background-fill rects to `items` (so
/// they render beneath the text). The box spans the full indented width and the
/// paragraph height. Background is inserted last so it sits behind the border.
fn prepend_para_box(
    items: &mut Vec<PositionedItem>,
    para_props: &ResolvedParaProps,
    width: f32,
    height: f32,
) {
    let bw = width + para_props.indent_start + para_props.indent_end;
    let has_border = para_props.border_top.is_some()
        || para_props.border_right.is_some()
        || para_props.border_bottom.is_some()
        || para_props.border_left.is_some();
    if has_border {
        items.insert(
            0,
            PositionedItem::BorderRect(PositionedBorderRect {
                rect: LayoutRect::new(0.0, 0.0, bw, height),
                top: para_props.border_top,
                right: para_props.border_right,
                bottom: para_props.border_bottom,
                left: para_props.border_left,
            }),
        );
    }
    if let Some(bg) = para_props.background_color {
        items.insert(
            0,
            PositionedItem::FilledRect(PositionedRect {
                rect: LayoutRect::new(0.0, 0.0, bw, height),
                color: bg,
            }),
        );
    }
}

/// Lays out a single paragraph using Parley, without consulting or populating
/// the shaping cache. [`layout_paragraph`] wraps this with memoisation.
// One more argument than the 7-arg limit: the optional spell checker threads
// alongside the existing shaping inputs (see `layout_paragraph_spelled`).
#[allow(clippy::too_many_arguments)]
fn layout_paragraph_uncached(
    resources: &mut FontResources,
    text_content: &str,
    style_spans: &[StyleSpan],
    para_props: &ResolvedParaProps,
    available_width: f32,
    display_scale: f32,
    preserve_for_editing: bool,
    spell: Option<&crate::SpellState>,
) -> ParagraphLayout {
    let (mut clean_text, mut clean_spans, mut orig_to_clean, mut clean_to_orig) =
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
                drop_lines: 0,
                drop_shift: 0.0,
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
            drop_lines: 0,
            drop_shift: 0.0,
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

    // Byte offset of the first decimal separator after each tab (before the
    // next tab / end), for Decimal-aligned stops.
    let decimal_positions: Vec<Option<usize>> = tab_char_positions
        .iter()
        .enumerate()
        .map(|(i, &t)| {
            let end = tab_char_positions
                .get(i + 1)
                .copied()
                .unwrap_or(clean_text.len());
            clean_text[t + 1..end].find('.').map(|rel| t + 1 + rel)
        })
        .collect();

    let tab_plans: Vec<tabs::TabPlan> = if tab_char_positions.is_empty() {
        vec![]
    } else {
        let n = tab_char_positions.len();
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
            if let Some(dpos) = decimal_positions[idx] {
                probe.push_inline_box(InlineBox {
                    id: DEC_ID_BASE + idx as u64,
                    kind: InlineBoxKind::InFlow,
                    index: dpos,
                    width: 0.0,
                    height: 0.0,
                });
            }
        }
        probe.push_inline_box(InlineBox {
            id: END_ID,
            kind: InlineBoxKind::InFlow,
            index: clean_text.len(),
            width: 0.0,
            height: 0.0,
        });
        push_math_inline_boxes(&mut probe, &math_boxes);
        let mut probe_layout = probe.build(&clean_text);
        probe_layout.break_all_lines(Some(line_w));

        let mut x_tab = vec![0.0f32; n];
        let mut line_tab = vec![usize::MAX; n];
        let mut x_dec = vec![f32::NAN; n];
        let mut x_end = 0.0f32;
        let mut line_end = usize::MAX;
        for (li, line) in probe_layout.lines().enumerate() {
            for item in line.items() {
                if let PositionedLayoutItem::InlineBox(pib) = item {
                    let id = pib.id;
                    if (id as usize) < n {
                        x_tab[id as usize] = pib.x;
                        line_tab[id as usize] = li;
                    } else if (DEC_ID_BASE..END_ID).contains(&id) {
                        let i = (id - DEC_ID_BASE) as usize;
                        if i < n {
                            x_dec[i] = pib.x;
                        }
                    } else if id == END_ID {
                        x_end = pib.x;
                        line_end = li;
                    }
                }
            }
        }
        tabs::compute_tab_plans(
            &para_props.tab_stops,
            para_props.indent_hanging,
            &x_tab,
            &line_tab,
            &x_dec,
            x_end,
            line_end,
        )
    };

    // ── Drop-cap preparation ──────────────────────────────────────────────────
    // The dropped initial spans several lines, so it is removed from the body
    // flow and rendered separately; the first `n_lines` body lines are narrowed
    // and shifted to clear it. The cap bytes are trimmed from `clean_text`, so
    // the orig↔clean maps are rebased past them below to keep editor hit-testing
    // aligned. Read-only paint uses the precise two-pass band split; the editor
    // (`preserve_for_editing`) renders the same enlarged cap but lays the body
    // out as a single uniform-narrow layout it can hit-test against (the lines
    // below the cap are slightly narrow, as documented). Tabs / inline math
    // disqualify a paragraph (the cap's manual breaking is incompatible).
    let drop_state: Option<(
        loki_doc_model::style::props::drop_cap::DropCap,
        String,
        StyleSpan,
    )> = para_props
        .drop_cap
        .filter(|_| tab_char_positions.is_empty() && math_boxes.is_empty())
        .and_then(|dc| {
            let k = crate::para_drop_cap::cap_byte_len(&clean_text, dc.length);
            if k == 0 || k >= clean_text.len() {
                return None; // no initial, or no body text would remain
            }
            let base = clean_spans
                .iter()
                .find(|s| s.range.start == 0 && s.range.end > 0)
                .or_else(|| clean_spans.first())
                .cloned()?;
            let cap_text = clean_text[..k].to_string();
            let (body, body_spans) =
                crate::para_drop_cap::trim_leading(&clean_text, &clean_spans, k);
            clean_text = body;
            clean_spans = body_spans;
            Some((dc, cap_text, base))
        });

    // Rebase the orig↔clean maps past the trimmed cap so the body layout's
    // offsets (which start after the cap) map back to the right original bytes
    // for editor hit-testing. The cap bytes [0, k) collapse to body offset 0;
    // body byte j corresponds to clean byte j + k.
    let drop_cap_bytes = drop_state
        .as_ref()
        .map(|(_, cap, _)| cap.len())
        .unwrap_or(0);
    if drop_cap_bytes > 0 {
        for v in orig_to_clean.iter_mut() {
            *v = v.saturating_sub(drop_cap_bytes);
        }
        let drain_to = drop_cap_bytes.min(clean_to_orig.len());
        clean_to_orig.drain(0..drain_to);
    }

    // ── Main (final) layout pass ──────────────────────────────────────────────
    let mut builder = resources.layout_cx.ranged_builder(
        &mut resources.font_cx,
        &clean_text,
        display_scale,
        true,
    );
    push_para_styles(&mut builder, para_props, &clean_spans);
    for (idx, &pos) in tab_char_positions.iter().enumerate() {
        let width = tab_plans.get(idx).map(|p| p.width).unwrap_or(0.0);
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
    // Plan the drop cap (its enlarged glyph + band geometry) from the body's
    // first-line metrics. `drop_plan` keeps the line height for `cover_height`.
    let drop_plan = if let Some((dc, cap_text, base)) = &drop_state {
        layout.break_all_lines(Some(line_w)); // metrics only
        let (lh, asc, bl) = layout
            .lines()
            .next()
            .map(|l| {
                let m = l.metrics();
                (m.line_height, m.ascent, m.baseline)
            })
            .unwrap_or((0.0, 0.0, 0.0));
        crate::para_drop_cap::plan_drop_cap(
            resources,
            cap_text,
            base,
            dc,
            lh,
            bl,
            asc,
            display_scale,
        )
        .map(|p| (p, lh))
    } else {
        None
    };

    // Unified leading band: a drop cap (object on the left) or a float band set
    // by the flow engine. The band's first lines are narrowed; lines below it
    // reclaim full width (`para_band` lays the body out in two passes).
    let band: Option<crate::para_band::Band> = if let Some((p, lh)) = &drop_plan {
        Some(crate::para_band::Band {
            inset: p.body_inset,
            cover_height: p.n_lines as f32 * lh,
            // In-text drop shifts the text right; margin drop has inset 0.
            shift_text: p.body_inset > 0.0,
        })
    } else {
        para_props.wrap_band.map(|w| crate::para_band::Band {
            inset: w.inset,
            cover_height: w.cover_height,
            shift_text: w.shift_text,
        })
    };

    // Precise per-line band split runs on the read-only paint path for plain
    // text; the editor / tab / math paths fall back to a uniform narrow below.
    let can_split = !preserve_for_editing && tab_char_positions.is_empty() && math_boxes.is_empty();

    if let Some(band) = band.as_ref().filter(|_| can_split) {
        let body = crate::para_band::layout_band_body(
            resources,
            &clean_text,
            &clean_spans,
            para_props,
            line_w,
            display_scale,
            band,
        );
        let mut items = body.items;
        let mut content_bottom = body.height;
        if let Some((p, _)) = &drop_plan {
            // Emit the enlarged initial at the paragraph's left edge.
            for it in &p.items {
                let mut it = it.clone();
                it.translate(para_props.indent_start, 0.0);
                items.push(it);
            }
            content_bottom = content_bottom.max(p.bottom);
        }
        prepend_para_box(&mut items, para_props, body.width, body.height);
        return ParagraphLayout {
            height: content_bottom,
            width: body.width,
            items,
            first_baseline: body.first_baseline,
            last_baseline: body.last_baseline,
            line_boundaries: body.line_boundaries,
            parley_layout: None,
            orig_to_clean,
            clean_to_orig,
            indent_start: para_props.indent_start,
            indent_hanging: para_props.indent_hanging,
            drop_lines: 0,
            drop_shift: 0.0,
        };
    }

    // Fallback / normal path: break at the (possibly band-narrowed) width. A
    // band here is a drop cap in the editor, or a float that could not be split
    // (editor, tabs, or math). Every line wraps at the narrowed width (APPROX,
    // as documented for `para_band`); only the leading lines beside the object
    // are shifted right to clear it.
    let band_inset = band.as_ref().map(|b| b.inset).unwrap_or(0.0);
    let drop_shift = band
        .as_ref()
        .map(|b| if b.shift_text { b.inset } else { 0.0 })
        .unwrap_or(0.0);
    layout.break_all_lines(Some((line_w - band_inset).max(1.0)));
    layout.align(para_props.alignment, AlignmentOptions::default());

    // Leading lines whose top is within the band's vertical extent are shifted.
    let drop_lines = match &band {
        Some(b) => layout
            .lines()
            .take_while(|l| l.metrics().block_min_coord < b.cover_height)
            .count(),
        None => 0,
    };

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

    // OOXML lineRule="exact" (ODF fixed line height): the line box is a fixed
    // height and content taller than it is clipped — unlike "atLeast", which
    // grows. Each line's items are wrapped in a clip layer sized to the exact
    // line box so over-tall glyphs / inline objects are cut off as in Word.
    let exact_line_pts = match para_props.line_height {
        Some(ResolvedLineHeight::Exact(pts)) => Some(pts),
        _ => None,
    };

    // Highlight/background underlay (gap #10) and spelling squiggles: resolved
    // via Parley selection geometry and emitted behind the glyph runs.
    underlays::emit_highlight_underlays(
        &mut items,
        &layout,
        &clean_spans,
        para_props,
        drop_lines,
        drop_shift,
    );
    underlays::emit_spelling_squiggles(
        &mut items,
        &layout,
        &clean_text,
        spell,
        para_props,
        drop_lines,
        drop_shift,
    );

    for line in layout.lines() {
        // Index into `items` where this line's emitted items begin (used to wrap
        // them in a clip layer for exact line height).
        let line_item_start = items.len();
        // Hanging indent: the first line shifts left so the marker is visible to
        // the left of `indent_start`. Subsequent lines use the full `indent_start`.
        let mut indent_x = if line_index == 0 && para_props.indent_hanging > 0.0 {
            para_props.indent_start - para_props.indent_hanging
        } else {
            para_props.indent_start
        };
        // Leading lines beside a drop cap / float band are shifted right to
        // clear it; lines below it return to the paragraph's left edge.
        if line_index < drop_lines {
            indent_x += drop_shift;
        }
        let line_baseline = line.metrics().baseline;
        // Extra horizontal offset accumulated from horizontally-scaled (w:w)
        // runs earlier on this line, so later items shift right by the width the
        // scaling added instead of overlapping. Reset per line.
        let mut extra_x = 0.0f32;
        for item in line.items() {
            // Math inline box: emit the typeset equation's draw items, offset to
            // the box's resolved position on the line.
            if let PositionedLayoutItem::InlineBox(pib) = &item {
                if pib.id >= MATH_ID_BASE {
                    let mi = (pib.id - MATH_ID_BASE) as usize;
                    if let Some((_, render)) = math_boxes.get(mi) {
                        for prim in &render.items {
                            let mut prim = prim.clone();
                            prim.translate(pib.x + indent_x + extra_x, pib.y);
                            items.push(prim);
                        }
                        // The box top is at `pib.y` and its baseline at
                        // `pib.y + ascent`; the descent hangs below that.
                        content_bottom = content_bottom.max(pib.y + render.ascent + render.descent);
                    }
                } else if (pib.id as usize) < tab_char_positions.len() {
                    // Tab inline box: draw the stop's leader (if any) across the
                    // gap the box opened.
                    if let Some(plan) = tab_plans.get(pib.id as usize) {
                        tabs::emit_tab_leader(
                            &mut items,
                            plan.leader,
                            pib.x + indent_x + extra_x,
                            pib.x + indent_x + extra_x + pib.width,
                            line_baseline,
                        );
                    }
                }
                continue;
            }
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };
            let scale =
                span_scale_for_range(&clean_spans, glyph_run.run().text_range()).unwrap_or(1.0);
            // Reserve the extra width the run rendered (scaling, per-glyph or
            // uniform) so later runs on the line do not overlap.
            extra_x += crate::para_emit::emit_glyph_run(
                &glyph_run,
                indent_x + extra_x,
                &clean_spans,
                scale,
                resources,
                &mut items,
                // Highlights are emitted by the selection-geometry pass below.
                false,
            );
        }
        if let Some(pts) = exact_line_pts {
            // Clip this line's items to its fixed-height box. The clip is wide
            // horizontally (exact governs the vertical extent only; horizontal
            // overflow is handled by margins/wrapping, as in Word) and exactly
            // `pts` tall.
            //
            // Word anchors the exact line box at the BOTTOM of the text: the box
            // bottom sits at the baseline + descent and the top is `pts` above
            // it, so when the font is taller than `pts` the ascenders (and a
            // raised superscript) are clipped while descenders are preserved —
            // the well-known "tops cut off" behaviour of small exact spacing.
            // (A symmetric/centered box would instead clip descenders too, which
            // does not match Word.) Consecutive boxes still tile exactly because
            // Parley advances the baseline by `pts`.
            let lm = line.metrics();
            let top = lm.baseline + lm.descent - pts;
            let clipped: Vec<PositionedItem> = items.split_off(line_item_start);
            items.push(PositionedItem::ClippedGroup {
                clip_rect: LayoutRect::new(-line_w, top, line_w * 3.0, pts),
                items: clipped,
            });
        }
        line_index += 1;
    }

    // A drop cap reaches this fallback only in the editor (`preserve_for_editing`),
    // where the body is one hit-testable layout; emit its enlarged initial at the
    // paragraph's left edge, above the shifted body lines.
    if let Some((p, _)) = &drop_plan {
        for it in &p.items {
            let mut it = it.clone();
            it.translate(para_props.indent_start, 0.0);
            items.push(it);
        }
        content_bottom = content_bottom.max(p.bottom);
    }

    prepend_para_box(&mut items, para_props, total_width, total_height);

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
        drop_lines,
        drop_shift,
    }
}

// List-marker synthesis lives in `crate::list_marker` (split from this
// file); re-exported here so `para::format_counter` callers and the
// `para_tests.rs` suite keep their existing paths.
pub(crate) use crate::list_marker::format_counter;
pub use crate::list_marker::format_list_marker;

// ── Private helpers for span → glyph-run lookups ──────────────────────────────

/// Returns the highlight colour for the first span fully containing
/// `text_range`, or `None` if no such span has a highlight.
/// Returns the span whose byte range contains `offset`, or `None` if no span
/// covers it. Empty (zero-width) spans never match. Used by per-glyph emission
/// to resolve each glyph's scale / baseline shift.
pub(crate) fn span_at_offset(spans: &[StyleSpan], offset: usize) -> Option<&StyleSpan> {
    spans
        .iter()
        .find(|s| s.range.start <= offset && offset < s.range.end)
}

pub(crate) fn span_highlight_for_range(
    spans: &[StyleSpan],
    text_range: Range<usize>,
) -> Option<LayoutColor> {
    spans
        .iter()
        .find(|s| s.range.start <= text_range.start && s.range.end >= text_range.end)
        .and_then(|s| s.highlight_color)
}

/// Returns the horizontal text scale for the first span fully containing
/// `text_range`, or `None` when the run is unscaled (100 %).
pub(crate) fn span_scale_for_range(spans: &[StyleSpan], text_range: Range<usize>) -> Option<f32> {
    spans
        .iter()
        .find(|s| s.range.start <= text_range.start && s.range.end >= text_range.end)
        .and_then(|s| s.scale)
}

/// Returns the link URL for the first span fully containing `text_range`,
/// or `None` if no span in that range carries a link URL.
pub(crate) fn span_link_url_for_range(
    spans: &[StyleSpan],
    text_range: Range<usize>,
) -> Option<String> {
    spans
        .iter()
        .find(|s| s.range.start <= text_range.start && s.range.end >= text_range.end)
        .and_then(|s| s.link_url.clone())
}

/// Returns `true` if the first span fully containing `text_range` has
/// `shadow = true`.
pub(crate) fn span_has_shadow(spans: &[StyleSpan], text_range: Range<usize>) -> bool {
    spans
        .iter()
        .find(|s| s.range.start <= text_range.start && s.range.end >= text_range.end)
        .is_some_and(|s| s.shadow)
}

/// Returns the vertical alignment and original (pre-reduction) font size for
/// the first span fully containing `text_range`, or `None` if no vertical
/// alignment is set on that span.
pub(crate) fn span_vertical_align_for_range(
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
