// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Style resolution — bridges `loki-doc-model` types to the renderer-agnostic
//! layout types.
//!
//! The public functions take a [`StyledParagraph`] / [`StyledRun`] plus a
//! [`StyleCatalog`] and produce the flattened representations consumed by
//! [`crate::para::layout_paragraph`].
//!
//! Inline images have no Parley inline-box representation: `walk_inlines`
//! collects each `Inline::Image` (src / EMU dimensions / alt / float wrap) as
//! a [`CollectedImage`] and the flow engine places it after text layout —
//! block-interruption placement for inline drawings, `flow_float` for
//! floating ones. Links ride [`StyleSpan::link_url`] (feature 5.11).

use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::{NoteKind, StyledRun};
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::props::border::{Border as DocBorder, BorderStyle as DocBorderStyle};
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::ParaProps;
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;

use crate::color::LayoutColor;
use crate::items::{BorderEdge, BorderStyle};
use crate::para::{ResolvedParaProps, StyleSpan};

#[path = "resolve_char_span.rs"]
mod char_span;
#[path = "resolve_inlines.rs"]
mod inlines;
#[path = "para_props_map.rs"]
pub(crate) mod para_map;
#[path = "resolve_walk.rs"]
mod walk;

use char_span::{char_props_to_style_span, effective_run_char_props};
pub use inlines::flatten_paragraph_with_base;

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
/// Resolution order (child wins): named style chain via
/// [`StyleCatalog::resolve_para`], then direct formatting on the paragraph.
pub fn resolve_para_props(block: &StyledParagraph, catalog: &StyleCatalog) -> ResolvedParaProps {
    let mut base: ParaProps = catalog
        .effective_paragraph_style(block.style_id.as_ref())
        .and_then(|id| catalog.resolve_para(id))
        .unwrap_or_default();
    if let Some(direct) = &block.direct_para_props {
        base = direct.as_ref().clone().merged_with_parent(&base);
    }
    let mut resolved = para_map::map_para_props(&base);
    resolved.para_mark_deleted_color = crate::revision_style::para_mark_deletion_color(block);
    resolved
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
    flatten_paragraph_with_base(block, catalog, note_counter, None)
}

// ── Border conversion ──────────────────────────────────────────────────────────
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

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
