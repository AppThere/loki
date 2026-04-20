// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Style resolution — bridges `loki-doc-model` types to the renderer-agnostic
//! layout types.
//!
//! The public functions take a [`StyledParagraph`] / [`StyledRun`] plus a
//! [`StyleCatalog`] and produce the flattened representations consumed by
//! [`crate::para::layout_paragraph`].
//!
//! # Session 4 pre-audit findings (2026-04-20)
//!
//! ## Q1 — Inline::Image data resolution
//!
//! `Inline::Image` is defined as `Image(NodeAttr, Vec<Inline>, LinkTarget)`
//! where `LinkTarget.url` carries either:
//! - A **data URI** (`"data:image/png;base64,…"`) when
//!   `DocxImportOptions::embed_images == true` (the default). The OOXML mapper
//!   (`loki-ooxml/src/docx/mapper/images.rs:38–44`) resolves the `a:blip
//!   r:embed` relationship ID against the OPC package, base64-encodes the raw
//!   bytes, and stores the data URI in `LinkTarget.url`.
//! - An **unresolved relationship ID** (e.g. `"rId1"`) when
//!   `embed_images == false`.
//! Width and height in EMUs are stored in `NodeAttr.kv` as `("cx_emu", val)`
//! and `("cy_emu", val)`. Alt text is the `Vec<Inline>` second field.
//! `loki-vello` (`loki-vello/src/image.rs:34`) only renders data URIs; external
//! URLs produce a grey placeholder rectangle. **With `embed_images` on (the
//! default), image data is available at layout time — Session 4 is a one-session
//! wire-up.**
//!
//! `walk_inlines` currently treats `Inline::Image` like `Inline::Link` — it
//! recurses into the alt-text child inlines and silently discards the `src` and
//! dimensions (`resolve.rs:351`). The fix is to extract `LinkTarget.url`,
//! `cx_emu`/`cy_emu`, and emit a `PositionedImage` into the paragraph's item
//! list. `PositionedItem::Image(PositionedImage)` already exists in
//! `loki-layout/src/items.rs:30` with fields `rect: LayoutRect`, `src: String`,
//! `alt: Option<String>`.
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

use loki_doc_model::content::block::StyledParagraph;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::props::border::{Border as DocBorder, BorderStyle as DocBorderStyle};
use loki_doc_model::style::props::char_props::{
    CharProps,
    StrikethroughStyle as DocStrikethroughStyle,
    UnderlineStyle as DocUnderlineStyle,
    VerticalAlign as DocVerticalAlign,
};
use loki_doc_model::style::list_style::ListId;
use loki_doc_model::style::props::para_props::{
    LineHeight as DocLineHeight, ParagraphAlignment, ParaProps, Spacing,
};
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;
use parley::Alignment;

use crate::color::LayoutColor;
use crate::geometry::LayoutInsets;
use crate::items::{BorderEdge, BorderStyle};
use crate::para::{
    FontVariant, ResolvedLineHeight, ResolvedListMarker, ResolvedParaProps, StrikethroughStyle,
    StyleSpan, UnderlineStyle, VerticalAlign,
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

/// Flatten all inline content of a [`StyledParagraph`] into a UTF-8 string,
/// a matching list of [`StyleSpan`]s, and any inline images found.
///
/// Each span's `range` is a byte range within the returned string.
/// Images are returned separately because Parley has no inline image support;
/// they are placed by the flow engine after text layout.
pub fn flatten_paragraph(
    block: &StyledParagraph,
    catalog: &StyleCatalog,
) -> (String, Vec<StyleSpan>, Vec<CollectedImage>) {
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
    let mut images: Vec<CollectedImage> = Vec::new();
    walk_inlines(&block.inlines, &base, catalog, &mut buf, &mut spans, None, &mut images);
    (buf, spans, images)
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
//
// Fields SILENTLY DROPPED (out of scope for Group 1):
//   font_name_complex    — complex-script font (BiDi)
//   font_name_east_asian — East Asian font
//   font_size_complex    — complex-script font size
//   background_color     — per-run background (distinct from highlight)
//   outline              — hollow text effect
//   scale                — horizontal text scale (gap #14, P2)
//   kerning              — kerning flag (gap #23, P3)
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
    let highlight_color = map_highlight_color(props.highlight_color);

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

    StyleSpan {
        range,
        font_name: props.font_name.clone(),
        font_size: props.font_size.map(pts_to_f32).unwrap_or(12.0),
        bold: props.bold.unwrap_or(false),
        italic: props.italic.unwrap_or(false),
        color: resolve_color(props.color.as_ref()),
        underline,
        strikethrough,
        line_height: None,
        vertical_align,
        highlight_color,
        letter_spacing: props.letter_spacing.map(pts_to_f32), // gap #13
        font_variant,
        word_spacing: props.word_spacing.map(pts_to_f32),     // gap #22
        shadow: props.shadow.unwrap_or(false),                 // gap #24
        link_url: None, // set by walk_inlines when inside Inline::Link (gap #11)
    }
}

/// Convert a [`HighlightColor`] palette entry to a [`LayoutColor`].
///
/// Returns `None` for [`HighlightColor::None`] (explicit highlight removal).
fn map_highlight_color(hc: Option<loki_doc_model::style::props::char_props::HighlightColor>) -> Option<LayoutColor> {
    use loki_doc_model::style::props::char_props::HighlightColor::*;
    match hc? {
        Yellow      => Some(LayoutColor::new(1.000, 1.000, 0.000, 1.0)),
        Green       => Some(LayoutColor::new(0.000, 1.000, 0.000, 1.0)),
        Cyan        => Some(LayoutColor::new(0.000, 1.000, 1.000, 1.0)),
        Magenta     => Some(LayoutColor::new(1.000, 0.000, 1.000, 1.0)),
        Blue        => Some(LayoutColor::new(0.000, 0.000, 1.000, 1.0)),
        Red         => Some(LayoutColor::new(1.000, 0.000, 0.000, 1.0)),
        DarkBlue    => Some(LayoutColor::new(0.000, 0.000, 0.502, 1.0)),
        DarkCyan    => Some(LayoutColor::new(0.000, 0.502, 0.502, 1.0)),
        DarkGreen   => Some(LayoutColor::new(0.000, 0.502, 0.000, 1.0)),
        DarkMagenta => Some(LayoutColor::new(0.502, 0.000, 0.502, 1.0)),
        DarkRed     => Some(LayoutColor::new(0.502, 0.000, 0.000, 1.0)),
        DarkYellow  => Some(LayoutColor::new(0.502, 0.502, 0.000, 1.0)),
        DarkGray    => Some(LayoutColor::new(0.502, 0.502, 0.502, 1.0)),
        LightGray   => Some(LayoutColor::new(0.753, 0.753, 0.753, 1.0)),
        Black       => Some(LayoutColor::BLACK),
        White       => Some(LayoutColor::WHITE),
        None        => Option::None,
        _           => Option::None,
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
    let start = buf.len();
    if props.all_caps == Some(true) {
        buf.push_str(&text.to_uppercase());
    } else {
        buf.push_str(text);
    }
    let mut span = char_props_to_style_span(props, start..buf.len());
    if let Some(url) = active_link_url {
        span.link_url = Some(url.to_string());
        // Auto-underline link text that has no explicit underline decoration.
        if span.underline.is_none() {
            span.underline = Some(UnderlineStyle::Single);
        }
    }
    spans.push(span);
}

/// Recursively collect text from an [`Inline`] tree, building `buf` + `spans`.
///
/// `active_link_url` carries the URL of the enclosing `Inline::Link`, if any;
/// it is threaded through recursive calls so all text inside a link gets
/// `StyleSpan::link_url` set. `images` collects any `Inline::Image` nodes
/// encountered for post-Parley placement (gap #9).
fn walk_inlines(
    inlines: &[Inline],
    effective: &CharProps,
    catalog: &StyleCatalog,
    buf: &mut String,
    spans: &mut Vec<StyleSpan>,
    active_link_url: Option<&str>,
    images: &mut Vec<CollectedImage>,
) {
    for inline in inlines {
        match inline {
            Inline::Str(s) => push_text(buf, spans, s, effective, active_link_url),
            Inline::Space => push_text(buf, spans, " ", effective, active_link_url),
            Inline::SoftBreak => push_text(buf, spans, " ", effective, active_link_url),
            Inline::LineBreak => push_text(buf, spans, "\n", effective, active_link_url),
            Inline::Code(_, s) => push_text(buf, spans, s, effective, active_link_url),
            Inline::StyledRun(run) => {
                let p = effective_run_char_props(run, catalog, effective);
                walk_inlines(&run.content, &p, catalog, buf, spans, active_link_url, images);
            }
            Inline::Strong(ch) => {
                let mut p = effective.clone();
                p.bold = Some(true);
                walk_inlines(ch, &p, catalog, buf, spans, active_link_url, images);
            }
            Inline::Emph(ch) => {
                let mut p = effective.clone();
                p.italic = Some(true);
                walk_inlines(ch, &p, catalog, buf, spans, active_link_url, images);
            }
            Inline::Underline(ch) => {
                let mut p = effective.clone();
                p.underline = Some(DocUnderlineStyle::Single);
                walk_inlines(ch, &p, catalog, buf, spans, active_link_url, images);
            }
            Inline::Strikeout(ch) => {
                let mut p = effective.clone();
                p.strikethrough = Some(DocStrikethroughStyle::Single);
                walk_inlines(ch, &p, catalog, buf, spans, active_link_url, images);
            }
            // Superscript (gap #3): set vertical_align on the effective props.
            Inline::Superscript(ch) => {
                let mut p = effective.clone();
                p.vertical_align = Some(DocVerticalAlign::Superscript);
                walk_inlines(ch, &p, catalog, buf, spans, active_link_url, images);
            }
            // Subscript (gap #3): set vertical_align on the effective props.
            Inline::Subscript(ch) => {
                let mut p = effective.clone();
                p.vertical_align = Some(DocVerticalAlign::Subscript);
                walk_inlines(ch, &p, catalog, buf, spans, active_link_url, images);
            }
            // SmallCaps (gap #15): set small_caps so StyleSpan gets FontVariant::SmallCaps.
            Inline::SmallCaps(ch) => {
                let mut p = effective.clone();
                p.small_caps = Some(true);
                walk_inlines(ch, &p, catalog, buf, spans, active_link_url, images);
            }
            Inline::Quoted(_, ch) | Inline::Span(_, ch) => {
                walk_inlines(ch, effective, catalog, buf, spans, active_link_url, images);
            }
            // Link (gap #11): thread the resolved URL into child spans.
            // TODO(link-click): interactive hit-testing deferred; only visual hint rendered.
            Inline::Link(_, ch, target) => {
                let url = target.url.as_str();
                walk_inlines(ch, effective, catalog, buf, spans, Some(url), images);
            }
            // Image (gap #9): collect for post-Parley placement; do not emit text.
            // TODO(floating-image): check NodeAttr.classes for "floating"; deferred (gap #12).
            // TODO(inline-image-flow): Parley has no inline box support; images placed
            //   as block-level prefix after layout_paragraph returns.
            Inline::Image(attr, alt_inlines, target) => {
                let cx_emu = attr.kv.iter()
                    .find(|(k, _)| k == "cx_emu")
                    .and_then(|(_, v)| v.parse::<u64>().ok())
                    .unwrap_or(0);
                let cy_emu = attr.kv.iter()
                    .find(|(k, _)| k == "cy_emu")
                    .and_then(|(_, v)| v.parse::<u64>().ok())
                    .unwrap_or(0);
                // Flatten alt-text inlines into a plain string (no spans, not main text).
                let mut alt_buf = String::new();
                let mut alt_spans: Vec<StyleSpan> = Vec::new();
                let mut alt_images: Vec<CollectedImage> = Vec::new();
                walk_inlines(alt_inlines, effective, catalog, &mut alt_buf, &mut alt_spans, None, &mut alt_images);
                let alt = if alt_buf.is_empty() { None } else { Some(alt_buf) };
                if !target.url.is_empty() {
                    images.push(CollectedImage {
                        src: target.url.clone(),
                        alt,
                        cx_emu,
                        cy_emu,
                    });
                }
            }
            Inline::Cite(_, ch) => walk_inlines(ch, effective, catalog, buf, spans, active_link_url, images),
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
    }
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
