// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Character- and inline-level style value types for paragraph layout (split
//! from `para.rs` for the 300-line ceiling): the decoration-style enums, the
//! resolved list/tab/line-height descriptors, and the per-run [`StyleSpan`].
//! Paragraph-level properties and the layout result live in
//! `para_layout_types.rs`; both are re-exported from `para.rs`.

use std::ops::Range;

use loki_doc_model::style::list_style::ListId;
use loki_doc_model::style::props::tab_stop::{TabAlignment, TabLeader};

use crate::color::LayoutColor;

/// Vertical text position for superscript / subscript runs. Mirrors
/// [`loki_doc_model::style::props::char_props::VerticalAlign`].
/// TR 29166 §6.2.1. ODF `style:text-position`; OOXML `w:vertAlign`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlign {
    /// Text raised above the baseline (superscript).
    Superscript,
    /// Text lowered below the baseline (subscript).
    Subscript,
}

/// Caps variant for a text run. TR 29166 §6.2.1.
/// ODF `fo:font-variant` / `fo:text-transform`; OOXML `w:smallCaps` / `w:caps`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontVariant {
    /// Render lowercase letters as small capitals.
    SmallCaps,
    /// All characters uppercased (text transform applied at build time).
    AllCaps,
}

/// Underline decoration style, mirroring the doc-model enum. Rendered
/// per-variant (5.2): the emitter carries the variant onto the
/// `PositionedDecoration` and `loki-vello` strokes each style.
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

/// Strikethrough decoration style, mirroring the doc-model enum. Rendered
/// per-variant (5.2): `Double` maps to a double stroke, `Single` to one line.
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
    /// The variant is recovered by `para_emit` (via `span_underline`) and
    /// carried on the `PositionedDecoration` so `loki-vello` strokes it
    /// per-style — single / double / dotted / dashed / wave / thick (5.2).
    pub underline: Option<UnderlineStyle>,
    /// Strikethrough decoration style. `None` = no strikethrough.
    ///
    /// `Double` (`w:dstrike`) renders as a double stroke; `Single` as one (5.2).
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
    /// children. Renders the visual link hint and backs `link_at` hit-testing
    /// + Ctrl/Cmd+click opening (feature 5.11).
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

    /// BCP-47 language tag of this run (OOXML `w:lang`, ODF
    /// `fo:language`/`fo:country`), carried for per-run spell-check routing
    /// (gap #30) — see [`crate::SpellState::checker_for`]. `None` = untagged.
    pub language: Option<std::sync::Arc<str>>,
}
