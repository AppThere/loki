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
//!   Width and height in EMUs are stored in `NodeAttr.kv` as `("cx_emu", val)`
//!   and `("cy_emu", val)`. Alt text is the `Vec<Inline>` second field.
//!   `loki-vello` (`loki-vello/src/image.rs:34`) only renders data URIs; external
//!   URLs produce a grey placeholder rectangle. **With `embed_images` on (the
//!   default), image data is available at layout time — Session 4 is a one-session
//!   wire-up.**
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

mod char_props;
mod color;
mod inline_types;
mod inline_walk;
mod para_props;
mod units;

// Re-export public types.
pub use color::resolve_color;
pub use inline_types::{CollectedImage, CollectedNote};
pub use units::{emu_to_pt, pts_to_f32};

// Re-export crate-internal helpers used outside this module.
pub(crate) use para_props::convert_border;

// Re-exports for tests (resolve_tests.rs uses `use super::*`).
#[cfg(test)]
pub(crate) use crate::color::LayoutColor;
#[cfg(test)]
pub(crate) use char_props::char_props_to_style_span;

use loki_doc_model::content::block::StyledParagraph;
use loki_doc_model::content::inline::StyledRun;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::props::char_props::CharProps;

use crate::para::{ResolvedParaProps, StyleSpan};

/// Resolve the effective [`ResolvedParaProps`] for a [`StyledParagraph`].
///
/// Resolution order (child wins):
/// 1. Named style chain via [`StyleCatalog::resolve_para`].
/// 2. Direct paragraph formatting on the paragraph itself.
pub fn resolve_para_props(block: &StyledParagraph, catalog: &StyleCatalog) -> ResolvedParaProps {
    let mut base: loki_doc_model::style::props::para_props::ParaProps = block
        .style_id
        .as_ref()
        .and_then(|id| catalog.resolve_para(id))
        .unwrap_or_default();
    if let Some(direct) = &block.direct_para_props {
        base = direct.as_ref().clone().merged_with_parent(&base);
    }
    para_props::map_para_props(&base)
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
    char_props::char_props_to_style_span(
        &char_props::effective_run_char_props(run, catalog, para_char_defaults),
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
    let mut notes: Vec<CollectedNote> = Vec::new();
    inline_walk::walk_inlines(
        &block.inlines,
        &base,
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

#[cfg(test)]
#[path = "../resolve_tests.rs"]
mod tests;
