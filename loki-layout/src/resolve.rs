// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Style resolution — bridges `loki-doc-model` types to the renderer-agnostic
//! layout types.
//!
//! The public functions take a [`StyledParagraph`] / [`StyledRun`] plus a
//! [`StyleCatalog`] and produce the flattened representations consumed by
//! [`crate::para::layout_paragraph`].
//!
//! # Session 4 pre-audit findings (2026-04-20)
//!
//! ## Q1 — Inline::Image data resolution (implemented)
//!
//! `Image(NodeAttr, Vec<Inline>, LinkTarget)` carries the src in `LinkTarget.url`
//! (a `data:` URI when `embed_images`, else an unresolved rel ID) and
//! `cx_emu`/`cy_emu` in `NodeAttr.kv`; alt text is the `Vec<Inline>`. `loki-vello`
//! renders only data URIs (external URLs → grey placeholder). `walk_inlines`
//! emits a `CollectedImage`, placed post-Parley as a `PositionedImage`.
//!
//! ## Q2 — Inline::Link current behaviour
//!
//! `Inline::Link` is `Link(NodeAttr, Vec<Inline>, LinkTarget)`. The OOXML mapper
//! (`loki-ooxml/src/docx/mapper/inline.rs:57–64`) resolves `w:hyperlink
//! r:id` relationship IDs against `ctx.hyperlinks` to produce a resolved HTTP
//! URL; bookmark-only anchors become `"#anchor_name"`.
//! `walk_inlines` (`resolve.rs:351`) recurses into display children and discards
//! the URL — identical to the image arm. No `PositionedItem::Link` variant
//! exists; hyperlink metadata is completely lost after flattening.
//! Fixing gap #11 requires either: (a) adding `PositionedItem::Link` with a
//! URL-annotated byte-range rect produced after glyph layout, or (b) threading
//! URL metadata through `StyleSpan` and attaching it to glyph runs so the
//! renderer can emit clickable regions. Option (b) is simpler — `StyleSpan`
//! already carries per-run metadata; adding `link_url: Option<String>` keeps
//! the URL co-located with the text run that displays it.
//!
//! ## Q3 — Image placement model for inline images
//!
//! Inline images in OOXML are inline drawings (`<wp:inline>`) sized in EMUs.
//! Parley has no concept of inline image boxes; it lays out text only. The
//! practical placement strategy is: at `walk_inlines` time, when an
//! `Inline::Image` is encountered, emit a `PositionedImage` with its origin
//! relative to the paragraph top-left and dimensions converted from EMUs
//! (1 EMU = 1/914400 inch = 1/12700 pt). Because Parley has already been built
//! by the time the glyph-run loop runs, the image must be placed either
//! (a) before the paragraph text (treating the image as a block-level
//! interruption — crude but functional for most docs), or (b) using Parley's
//! inline-box callback if the API exposes it in a future version. For v0.1,
//! option (a) is used: collect images encountered during `walk_inlines` with
//! their EMU dimensions; after `layout_paragraph` returns, prepend
//! `PositionedItem::Image` entries to the paragraph's item list at `cursor_y`.
//! Floating drawings (`NodeAttr.classes` contains `"floating"`) are placed as
//! absolute overlays at `(0, 0)` within the page content area — also deferred
//! to a follow-up session (gap #12 partial).

use std::ops::Range;

use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::field::types::{Field, FieldKind};
use loki_doc_model::content::inline::{Inline, NoteKind, StyledRun};
use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::list_style::ListId;
use loki_doc_model::style::props::border::{Border as DocBorder, BorderStyle as DocBorderStyle};
use loki_doc_model::style::props::char_props::{
    CharProps, StrikethroughStyle as DocStrikethroughStyle, UnderlineStyle as DocUnderlineStyle,
    VerticalAlign as DocVerticalAlign,
};
use loki_doc_model::style::props::para_props::{
    LineHeight as DocLineHeight, ParaProps, ParagraphAlignment, Spacing,
};
use loki_doc_model::style::props::tab_stop::TabAlignment;
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;
use parley::Alignment;

use crate::color::LayoutColor;
use crate::geometry::LayoutInsets;
use crate::items::{BorderEdge, BorderStyle};
use crate::para::{
    FontVariant, ResolvedLineHeight, ResolvedListMarker, ResolvedParaProps, ResolvedTabStop,
    StrikethroughStyle, StyleSpan, UnderlineStyle, VerticalAlign,
};

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

/// Convert English Metric Units (EMU) to points. 1 EMU = 1/12700 pt.
///
/// OOXML stores image dimensions in EMU. This converts to the `f32` points
/// used by `loki-layout` geometry types.
pub fn emu_to_pt(emu: u64) -> f32 {
    emu as f32 / 12700.0
}

/// An inline image collected during paragraph flattening.
///
/// Gathered by [`flatten_paragraph`] / `walk_inlines` when an [`Inline::Image`]
/// is encountered. Passed back to the flow engine so it can emit a
/// [`crate::items::PositionedImage`] after Parley text layout.
#[derive(Debug, Clone)]
pub struct CollectedImage {
    /// Data URI (`"data:image/…;base64,…"`) or external URL.
    pub src: String,
    /// Alt text flattened from the image's alt-text inline children.
    pub alt: Option<String>,
    /// Width in English Metric Units.
    pub cx_emu: u64,
    /// Height in English Metric Units.
    pub cy_emu: u64,
    /// Float wrap configuration when the drawing is anchored (floating), or
    /// `None` for an inline drawing. Read from the image's `NodeAttr` (see
    /// [`loki_doc_model::content::float::FloatWrap`]).
    pub float: Option<loki_doc_model::content::float::FloatWrap>,
}

/// A footnote or endnote body collected during paragraph flattening.
///
/// Gathered by [`flatten_paragraph`] / `walk_inlines` when an [`Inline::Note`]
/// is encountered. Passed back to the flow engine so it can render the note
/// body at the end of the section.
#[derive(Debug, Clone)]
pub struct CollectedNote {
    /// Sequential note number within the section (1-based).
    pub number: u32,
    /// Whether this is a footnote or an endnote.
    pub kind: NoteKind,
    /// The note body blocks.
    pub blocks: Vec<Block>,
    /// Owning paragraph's global block index; set by `flow_paragraph` (0 until then).
    pub owner_block_index: usize,
    /// This note's index among its block's notes (the bridge `KEY_NOTES` index),
    /// so the editor can address the body via a `PathStep::Note`.
    pub note_in_block: usize,
}

/// Resolve the effective [`ResolvedParaProps`] for a [`StyledParagraph`].
///
/// Resolution order (child wins):
/// 1. Named style chain via [`StyleCatalog::resolve_para`].
/// 2. Direct paragraph formatting on the paragraph itself.
pub fn resolve_para_props(block: &StyledParagraph, catalog: &StyleCatalog) -> ResolvedParaProps {
    let mut base: ParaProps = catalog
        .effective_paragraph_style(block.style_id.as_ref())
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
    char_props_to_style_span(
        &effective_run_char_props(run, catalog, para_char_defaults),
        0..0,
    )
}

/// Flatten all inline content of a [`StyledParagraph`] into a UTF-8 string,
/// a matching list of [`StyleSpan`]s, inline images, and collected notes.
///
/// Each span's `range` is a byte range within the returned string.
/// Images are returned separately because Parley has no inline image support;
/// they are placed by the flow engine after text layout. Notes are rendered
/// at the end of the section by the flow engine.
///
/// `note_counter` is updated in place; the caller must pass the session-wide
/// counter so that note numbers are unique across paragraphs within a section.
pub fn flatten_paragraph(
    block: &StyledParagraph,
    catalog: &StyleCatalog,
    note_counter: &mut u32,
) -> (
    String,
    Vec<StyleSpan>,
    Vec<CollectedImage>,
    Vec<CollectedNote>,
) {
    let base: CharProps = catalog
        .effective_paragraph_style(block.style_id.as_ref())
        .and_then(|id| catalog.resolve_char(id))
        .unwrap_or_default();
    let base = match &block.direct_char_props {
        Some(direct) => direct.as_ref().clone().merged_with_parent(&base),
        None => base,
    };
    // `walk_inlines` takes `&mut` and mutates this in place (restoring after each
    // styled run) to avoid cloning `CharProps` per formatting span; `base` is a
    // throwaway local, so the mutation is not observable outside this call.
    let mut base = base;
    let mut buf = String::new();
    let mut spans: Vec<StyleSpan> = Vec::new();
    let mut images: Vec<CollectedImage> = Vec::new();
    let mut notes: Vec<CollectedNote> = Vec::new();
    walk_inlines(
        &block.inlines,
        &mut base,
        catalog,
        &mut buf,
        &mut spans,
        None,
        &mut images,
        note_counter,
        &mut notes,
    );
    (buf, spans, images, notes)
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Maximum number of parent links followed when resolving a character-style
/// chain. Guards against cyclic `parent` references in corrupt documents
/// (e.g. A.parent = B, B.parent = A). When the cap is exceeded, inheritance
/// stops — the chain is treated as if it ended at a root style.
const MAX_STYLE_CHAIN_DEPTH: usize = 32;

/// Walk the character-style parent chain in [`StyleCatalog::character_styles`].
fn resolve_char_style_chain(catalog: &StyleCatalog, id: &StyleId) -> CharProps {
    let Some(style) = catalog.character_styles.get(id) else {
        return CharProps::default();
    };
    let mut resolved = style.char_props.clone();
    let mut parent_id = style.parent.as_ref();
    for _ in 0..MAX_STYLE_CHAIN_DEPTH {
        let Some(parent) = parent_id.and_then(|pid| catalog.character_styles.get(pid)) else {
            break;
        };
        resolved = resolved.merged_with_parent(&parent.char_props);
        parent_id = parent.parent.as_ref();
    }
    resolved
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

// ── char_props_to_style_span ──────────────────────────────────────────────────
//
// Audit of CharProps → StyleSpan mapping (Group 1 gaps, 2026-04-20):
//
// Fields CURRENTLY MAPPED (pre-session):
//   font_name          → StyleSpan.font_name
//   font_size          → StyleSpan.font_size (default 12.0)
//   bold               → StyleSpan.bold
//   italic             → StyleSpan.italic
//   color              → StyleSpan.color
//   underline          → StyleSpan.underline (was bool; now Option<UnderlineStyle>)
//   strikethrough      → StyleSpan.strikethrough (was bool; now Option<StrikethroughStyle>)
//
// Fields ADDED in this session (Group 1 gaps):
//   vertical_align     → StyleSpan.vertical_align       (gap #3,  P0)
//   highlight_color    → StyleSpan.highlight_color      (gap #10, P1)
//   letter_spacing     → StyleSpan.letter_spacing       (gap #13, P2)
//   small_caps/all_caps→ StyleSpan.font_variant         (gap #15/#16, P2)
//   underline variant  → StyleSpan.underline Option     (gap #17, P2)
//   strikethrough var  → StyleSpan.strikethrough Option (gap #18, P2)
//   word_spacing       → StyleSpan.word_spacing         (gap #22, P3)
//   shadow             → StyleSpan.shadow               (gap #24, P3)
//   scale              → StyleSpan.scale                (gap #14, P2)
//
//   kerning            → StyleSpan.kerning (gap #23, P3; shaper toggle, default OFF)
//
// Fields SILENTLY DROPPED (out of scope for Group 1):
//   font_name_complex    — complex-script font (BiDi)
//   font_name_east_asian — East Asian font
//   font_size_complex    — complex-script font size
//   background_color     — per-run background (distinct from highlight)
//   outline              — hollow text effect
//   language / language_complex / language_east_asian — locale (gap #30, P3)
//   hyperlink            — URL (gap #11, P1 — handled at Inline level)

/// Convert a [`CharProps`] snapshot to a [`StyleSpan`] covering `range`.
fn char_props_to_style_span(props: &CharProps, range: Range<usize>) -> StyleSpan {
    // Superscript / subscript (gap #3): map to layout VerticalAlign.
    let vertical_align = match props.vertical_align {
        Some(DocVerticalAlign::Superscript) => Some(VerticalAlign::Superscript),
        Some(DocVerticalAlign::Subscript) => Some(VerticalAlign::Subscript),
        _ => None,
    };

    // Highlight colour (gap #10): convert named palette to LayoutColor.
    // Fall back to background_color (w:shd @fill on runs) when no named
    // highlight is set — both serve the same visual role.
    let highlight_color = map_highlight_color(props.highlight_color).or_else(|| {
        props
            .background_color
            .as_ref()
            .map(|c| resolve_color(Some(c)))
    });

    // Underline (gap #17): preserve variant (Parley renders all as single).
    let underline = match props.underline {
        Some(DocUnderlineStyle::Single) => Some(UnderlineStyle::Single),
        Some(DocUnderlineStyle::Double) => Some(UnderlineStyle::Double),
        Some(DocUnderlineStyle::Dotted) => Some(UnderlineStyle::Dotted),
        Some(DocUnderlineStyle::Dash) => Some(UnderlineStyle::Dash),
        Some(DocUnderlineStyle::Wave) => Some(UnderlineStyle::Wave),
        Some(DocUnderlineStyle::Thick) => Some(UnderlineStyle::Thick),
        None => None,
        // Non-exhaustive guard: future doc-model variants default to Single.
        _ => Some(UnderlineStyle::Single),
    };

    // Strikethrough (gap #18): preserve variant.
    let strikethrough = match props.strikethrough {
        Some(DocStrikethroughStyle::Single) => Some(StrikethroughStyle::Single),
        Some(DocStrikethroughStyle::Double) => Some(StrikethroughStyle::Double),
        None => None,
        _ => Some(StrikethroughStyle::Single),
    };

    // Caps variant (gaps #15, #16): small_caps takes precedence over all_caps.
    let font_variant = if props.small_caps == Some(true) {
        Some(FontVariant::SmallCaps)
    } else if props.all_caps == Some(true) {
        Some(FontVariant::AllCaps)
    } else {
        None
    };

    let bold = props.bold.unwrap_or(false);
    let mut span = StyleSpan {
        range,
        font_name: props.font_name.clone(),
        font_size: props.font_size.map(pts_to_f32).unwrap_or(12.0),
        bold,
        // Explicit numeric weight wins; otherwise derive from the bold flag.
        weight: props.font_weight.unwrap_or(if bold { 700 } else { 400 }),
        italic: props.italic.unwrap_or(false),
        color: resolve_color(props.color.as_ref()),
        underline,
        strikethrough,
        line_height: None,
        vertical_align,
        highlight_color,
        letter_spacing: props.letter_spacing.map(pts_to_f32), // gap #13
        font_variant,
        word_spacing: props.word_spacing.map(pts_to_f32), // gap #22
        shadow: props.shadow.unwrap_or(false),            // gap #24
        kerning: props.kerning,                           // gap #23
        link_url: None, // set by walk_inlines when inside Inline::Link (gap #11)
        math: None,     // set by walk_inlines for Inline::Math placeholders
        // Horizontal text scale (gap #14): only forward a non-trivial, positive
        // factor so the common 100 % case stays on the fast (unscaled) path.
        scale: props
            .scale
            .filter(|&s| s > 0.0 && (s - 1.0).abs() > f32::EPSILON),
        // Manual baseline rise (gap: w:position): forward only a non-zero shift.
        baseline_shift: props
            .baseline_shift
            .map(pts_to_f32)
            .filter(|&s| s.abs() > f32::EPSILON),
    };
    crate::revision_style::apply(&mut span, props);
    span
}

/// Convert a [`HighlightColor`] palette entry to a [`LayoutColor`].
///
/// Returns `None` for [`HighlightColor::None`] (explicit highlight removal).
fn map_highlight_color(
    hc: Option<loki_doc_model::style::props::char_props::HighlightColor>,
) -> Option<LayoutColor> {
    use loki_doc_model::style::props::char_props::HighlightColor::*;
    match hc? {
        Yellow => Some(LayoutColor::new(1.000, 1.000, 0.000, 1.0)),
        Green => Some(LayoutColor::new(0.000, 1.000, 0.000, 1.0)),
        Cyan => Some(LayoutColor::new(0.000, 1.000, 1.000, 1.0)),
        Magenta => Some(LayoutColor::new(1.000, 0.000, 1.000, 1.0)),
        Blue => Some(LayoutColor::new(0.000, 0.000, 1.000, 1.0)),
        Red => Some(LayoutColor::new(1.000, 0.000, 0.000, 1.0)),
        DarkBlue => Some(LayoutColor::new(0.000, 0.000, 0.502, 1.0)),
        DarkCyan => Some(LayoutColor::new(0.000, 0.502, 0.502, 1.0)),
        DarkGreen => Some(LayoutColor::new(0.000, 0.502, 0.000, 1.0)),
        DarkMagenta => Some(LayoutColor::new(0.502, 0.000, 0.502, 1.0)),
        DarkRed => Some(LayoutColor::new(0.502, 0.000, 0.000, 1.0)),
        DarkYellow => Some(LayoutColor::new(0.502, 0.502, 0.000, 1.0)),
        DarkGray => Some(LayoutColor::new(0.502, 0.502, 0.502, 1.0)),
        LightGray => Some(LayoutColor::new(0.753, 0.753, 0.753, 1.0)),
        Black => Some(LayoutColor::BLACK),
        White => Some(LayoutColor::WHITE),
        None => Option::None,
        _ => Option::None,
    }
}

/// Append `text` to `buf` and push a span; no-op for empty strings.
///
/// When `props.all_caps` is set, `text` is uppercased before appending
/// (gap #16 fallback — Parley has no `FontVariantCaps` property).
/// When `active_link_url` is `Some`, the span gets `link_url` set and an
/// auto-underline if not already underlined (gap #11).
#[inline]
fn push_text(
    buf: &mut String,
    spans: &mut Vec<StyleSpan>,
    text: &str,
    props: &CharProps,
    active_link_url: Option<&str>,
) {
    if text.is_empty() {
        return;
    }

    // Small caps (gap #15): Parley exposes no `FontVariantCaps`, so synthesize
    // it — every letter is uppercased and the letters that were *lowercase* in
    // the source render at a reduced size, the classic small-caps look. This
    // splits `text` into case-homogeneous segments (handled below). `all_caps`,
    // if also set, takes precedence and uses the plain uppercasing path.
    if props.small_caps == Some(true) && props.all_caps != Some(true) {
        push_small_caps(buf, spans, text, props, active_link_url);
        return;
    }

    let start = buf.len();
    if props.all_caps == Some(true) {
        buf.push_str(&text.to_uppercase());
    } else {
        buf.push_str(text);
    }
    let mut span = char_props_to_style_span(props, start..buf.len());
    apply_link(&mut span, active_link_url);
    spans.push(span);
}

/// Fraction of the full cap size used for synthesized small capitals (letters
/// that were lowercase in the source). Matches the common ~0.8 synthetic ratio
/// applied when a font carries no real small-cap (`smcp`) glyphs.
const SMALL_CAPS_RATIO: f32 = 0.8;

/// Append `text` as synthesized small caps: uppercase every letter, splitting
/// into runs where the source was lowercase (rendered at [`SMALL_CAPS_RATIO`] of
/// the size) versus already-uppercase / non-letters (full size). Each run is its
/// own [`StyleSpan`] so Parley shapes it at the right size.
fn push_small_caps(
    buf: &mut String,
    spans: &mut Vec<StyleSpan>,
    text: &str,
    props: &CharProps,
    active_link_url: Option<&str>,
) {
    let mut chars = text.chars().peekable();
    while let Some(&first) = chars.peek() {
        let lower = first.is_lowercase();
        let start = buf.len();
        while let Some(&c) = chars.peek() {
            if c.is_lowercase() != lower {
                break;
            }
            // Uppercase the character (may expand, e.g. ß → SS).
            for u in c.to_uppercase() {
                buf.push(u);
            }
            chars.next();
        }
        let mut span = char_props_to_style_span(props, start..buf.len());
        if lower {
            span.font_size *= SMALL_CAPS_RATIO;
        }
        apply_link(&mut span, active_link_url);
        spans.push(span);
    }
}

/// Apply an active hyperlink URL to `span`, auto-underlining undecorated link
/// text (gap #11).
fn apply_link(span: &mut StyleSpan, active_link_url: Option<&str>) {
    if let Some(url) = active_link_url {
        span.link_url = Some(url.to_string());
        if span.underline.is_none() {
            span.underline = Some(UnderlineStyle::Single);
        }
    }
}

/// Recursively collect text from an [`Inline`] tree, building `buf` + `spans`.
///
/// `active_link_url` carries the URL of the enclosing `Inline::Link`, if any;
/// it is threaded through recursive calls so all text inside a link gets
/// `StyleSpan::link_url` set. `images` collects any `Inline::Image` nodes
/// encountered for post-Parley placement (gap #9). `notes` collects footnotes
/// and endnotes; `note_counter` is incremented for each note (gap #2).
#[allow(clippy::too_many_arguments)]
fn walk_inlines(
    inlines: &[Inline],
    effective: &mut CharProps,
    catalog: &StyleCatalog,
    buf: &mut String,
    spans: &mut Vec<StyleSpan>,
    active_link_url: Option<&str>,
    images: &mut Vec<CollectedImage>,
    note_counter: &mut u32,
    notes: &mut Vec<CollectedNote>,
) {
    for inline in inlines {
        match inline {
            Inline::Str(s) => push_text(buf, spans, s, effective, active_link_url),
            Inline::Space => push_text(buf, spans, " ", effective, active_link_url),
            Inline::SoftBreak => push_text(buf, spans, " ", effective, active_link_url),
            Inline::LineBreak => push_text(buf, spans, "\n", effective, active_link_url),
            Inline::Code(_, s) => push_text(buf, spans, s, effective, active_link_url),
            Inline::StyledRun(run) => {
                let mut p = effective_run_char_props(run, catalog, effective);
                walk_inlines(
                    &run.content,
                    &mut p,
                    catalog,
                    buf,
                    spans,
                    active_link_url,
                    images,
                    note_counter,
                    notes,
                );
            }
            Inline::Strong(ch) => {
                // Toggle the single flag in place and restore it after recursing,
                // instead of cloning the whole CharProps (which heap-allocates its
                // font-name Strings) for every formatting span.
                let prev = effective.bold.replace(true);
                walk_inlines(
                    ch,
                    effective,
                    catalog,
                    buf,
                    spans,
                    active_link_url,
                    images,
                    note_counter,
                    notes,
                );
                effective.bold = prev;
            }
            Inline::Emph(ch) => {
                let prev = effective.italic.replace(true);
                walk_inlines(
                    ch,
                    effective,
                    catalog,
                    buf,
                    spans,
                    active_link_url,
                    images,
                    note_counter,
                    notes,
                );
                effective.italic = prev;
            }
            Inline::Underline(ch) => {
                let prev = effective.underline.replace(DocUnderlineStyle::Single);
                walk_inlines(
                    ch,
                    effective,
                    catalog,
                    buf,
                    spans,
                    active_link_url,
                    images,
                    note_counter,
                    notes,
                );
                effective.underline = prev;
            }
            Inline::Strikeout(ch) => {
                let prev = effective
                    .strikethrough
                    .replace(DocStrikethroughStyle::Single);
                walk_inlines(
                    ch,
                    effective,
                    catalog,
                    buf,
                    spans,
                    active_link_url,
                    images,
                    note_counter,
                    notes,
                );
                effective.strikethrough = prev;
            }
            // Superscript (gap #3): set vertical_align on the effective props.
            Inline::Superscript(ch) => {
                let prev = effective
                    .vertical_align
                    .replace(DocVerticalAlign::Superscript);
                walk_inlines(
                    ch,
                    effective,
                    catalog,
                    buf,
                    spans,
                    active_link_url,
                    images,
                    note_counter,
                    notes,
                );
                effective.vertical_align = prev;
            }
            // Subscript (gap #3): set vertical_align on the effective props.
            Inline::Subscript(ch) => {
                let prev = effective
                    .vertical_align
                    .replace(DocVerticalAlign::Subscript);
                walk_inlines(
                    ch,
                    effective,
                    catalog,
                    buf,
                    spans,
                    active_link_url,
                    images,
                    note_counter,
                    notes,
                );
                effective.vertical_align = prev;
            }
            // SmallCaps (gap #15): set small_caps so StyleSpan gets FontVariant::SmallCaps.
            Inline::SmallCaps(ch) => {
                let prev = effective.small_caps.replace(true);
                walk_inlines(
                    ch,
                    effective,
                    catalog,
                    buf,
                    spans,
                    active_link_url,
                    images,
                    note_counter,
                    notes,
                );
                effective.small_caps = prev;
            }
            Inline::Quoted(_, ch) | Inline::Span(_, ch) => {
                walk_inlines(
                    ch,
                    effective,
                    catalog,
                    buf,
                    spans,
                    active_link_url,
                    images,
                    note_counter,
                    notes,
                );
            }
            // Link (gap #11): thread the resolved URL into child spans.
            // TODO(link-click): interactive hit-testing deferred; only visual hint rendered.
            Inline::Link(_, ch, target) => {
                let url = target.url.as_str();
                walk_inlines(
                    ch,
                    effective,
                    catalog,
                    buf,
                    spans,
                    Some(url),
                    images,
                    note_counter,
                    notes,
                );
            }
            // Image (gap #9): collect for post-Parley placement; do not emit text.
            // TODO(floating-image): check NodeAttr.classes for "floating"; deferred (gap #12).
            // TODO(inline-image-flow): Parley has no inline box support; images placed
            //   as block-level prefix after layout_paragraph returns.
            Inline::Image(attr, alt_inlines, target) => {
                let cx_emu = attr
                    .kv
                    .iter()
                    .find(|(k, _)| k == "cx_emu")
                    .and_then(|(_, v)| v.parse::<u64>().ok())
                    .unwrap_or(0);
                let cy_emu = attr
                    .kv
                    .iter()
                    .find(|(k, _)| k == "cy_emu")
                    .and_then(|(_, v)| v.parse::<u64>().ok())
                    .unwrap_or(0);
                // Flatten alt-text inlines into a plain string (no spans, not main text).
                let mut alt_buf = String::new();
                let mut alt_spans: Vec<StyleSpan> = Vec::new();
                let mut alt_images: Vec<CollectedImage> = Vec::new();
                let mut _dummy_counter = 0u32;
                let mut _dummy_notes: Vec<CollectedNote> = Vec::new();
                walk_inlines(
                    alt_inlines,
                    effective,
                    catalog,
                    &mut alt_buf,
                    &mut alt_spans,
                    None,
                    &mut alt_images,
                    &mut _dummy_counter,
                    &mut _dummy_notes,
                );
                let alt = if alt_buf.is_empty() {
                    None
                } else {
                    Some(alt_buf)
                };
                if !target.url.is_empty() {
                    images.push(CollectedImage {
                        src: target.url.clone(),
                        alt,
                        cx_emu,
                        cy_emu,
                        float: loki_doc_model::content::float::FloatWrap::read(attr),
                    });
                }
            }
            Inline::Cite(_, ch) => walk_inlines(
                ch,
                effective,
                catalog,
                buf,
                spans,
                active_link_url,
                images,
                note_counter,
                notes,
            ),
            // Field (gap #4): emit current_value snapshot, or a kind-based fallback.
            Inline::Field(f) => {
                let text = field_display_text(f);
                if !text.is_empty() {
                    push_text(buf, spans, &text, effective, active_link_url);
                }
            }
            // Note (gap #2): emit a superscript reference mark and collect the body.
            Inline::Note(kind, blocks) => {
                *note_counter += 1;
                let mark = superscript_mark(*note_counter);
                let prev = effective
                    .vertical_align
                    .replace(DocVerticalAlign::Superscript);
                push_text(buf, spans, &mark, effective, active_link_url);
                effective.vertical_align = prev;
                notes.push(CollectedNote {
                    number: *note_counter,
                    kind: *kind,
                    blocks: blocks.clone(),
                    // Set by `flow_paragraph` after collection.
                    owner_block_index: 0,
                    note_in_block: 0,
                });
            }
            // Math (gap): record an empty-range placeholder span carrying the
            // MathML; `layout_paragraph` typesets it and places it inline via a
            // Parley inline box. No text is emitted into `buf`.
            Inline::Math(_, mathml) => {
                let at = buf.len();
                let mut span = char_props_to_style_span(effective, at..at);
                span.math = Some(std::sync::Arc::from(mathml.as_str()));
                spans.push(span);
            }
            // RawInline, Comment, Bookmark, and any future #[non_exhaustive]
            // variants are not text runs — skip.
            _ => {}
        }
    }
}

/// Return display text for a [`Field`]: use `current_value` if available,
/// otherwise fall back to a kind-specific placeholder.
fn field_display_text(f: &Field) -> String {
    if let Some(ref v) = f.current_value {
        return v.clone();
    }
    match &f.kind {
        FieldKind::PageNumber => "1".to_string(),
        FieldKind::PageCount => "1".to_string(),
        FieldKind::Date { .. } => String::new(),
        FieldKind::Time { .. } => String::new(),
        FieldKind::Title | FieldKind::Author | FieldKind::Subject | FieldKind::FileName => {
            String::new()
        }
        FieldKind::WordCount => String::new(),
        FieldKind::CrossReference { .. } => String::new(),
        FieldKind::Raw { .. } => String::new(),
        _ => String::new(),
    }
}

/// Return the Unicode superscript string for note number `n`.
///
/// Uses Unicode superscript digits (U+00B9, U+00B2, U+00B3, U+2074–U+2079)
/// for n ≤ 9, and `[n]` for larger numbers.
fn superscript_mark(n: u32) -> String {
    match n {
        1 => "\u{00B9}".to_string(),
        2 => "\u{00B2}".to_string(),
        3 => "\u{00B3}".to_string(),
        4 => "\u{2074}".to_string(),
        5 => "\u{2075}".to_string(),
        6 => "\u{2076}".to_string(),
        7 => "\u{2077}".to_string(),
        8 => "\u{2078}".to_string(),
        9 => "\u{2079}".to_string(),
        _ => format!("[{n}]"),
    }
}

/// Map a doc [`Border`][DocBorder] to a layout [`BorderEdge`], or `None` when
/// the border style is [`DocBorderStyle::None`].
pub(crate) fn convert_border(border: &DocBorder) -> Option<BorderEdge> {
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
        page_break_after: p.page_break_after.unwrap_or(false),
        // NOTE(bidi): `ParaProps.bidi` (RTL paragraph direction) is not forwarded.
        // Parley 0.6 has no `StyleProperty` for text direction and exposes no
        // public bidi level API (`BidiLevel`/`BidiResolver` are pub(crate)).
        // Parley runs BiDi automatically from Unicode character classes, so
        // purely RTL text in RTL scripts will display correctly without explicit
        // direction. Explicit `bidi: true` paragraphs in mixed-direction documents
        // may render incorrectly. Revisit when Parley exposes a direction API.
        // Tracked: fidelity audit gap #19 (deferred).
        indent_hanging: p.indent_hanging.map(pts_to_f32).unwrap_or(0.0),
        list_marker: match (&p.list_id, p.list_level) {
            (Some(id), Some(level)) => Some(ResolvedListMarker {
                list_id: ListId::new(id.as_str()),
                level,
            }),
            _ => None,
        },
        // Tab stops (gap #7): convert from Points to f32, sort ascending,
        // drop Clear entries (already filtered by the OOXML mapper).
        tab_stops: {
            let mut stops: Vec<ResolvedTabStop> = p
                .tab_stops
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .filter(|s| s.alignment != TabAlignment::Clear)
                .map(|s| ResolvedTabStop {
                    position: pts_to_f32(s.position),
                    alignment: s.alignment,
                    leader: s.leader,
                })
                .collect();
            stops.sort_by(|a, b| {
                a.position
                    .partial_cmp(&b.position)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            stops
        },
        // Set by the flow engine for table-cell content; see ResolvedParaProps.
        break_long_words: false,
        // Dropped initial (rendered in the read-only/paint path); see
        // `layout_paragraph`. Forwarded straight from the imported model.
        drop_cap: p.drop_cap,
        // Float wrap band is injected by the flow engine, not the model.
        wrap_band: None,
    }
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
