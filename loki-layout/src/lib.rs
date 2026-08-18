// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Renderer-agnostic layout engine for the Loki suite.
//!
//! `loki-layout` turns a [`loki_doc_model::Document`] into absolute positions
//! for all content — no GPU dependencies, fully testable without a display.
//!
//! # Layout Modes
//!
//! Three modes are supported via [`LayoutMode`]:
//!
//! - [`LayoutMode::Paginated`]: content broken into fixed-size pages.
//! - [`LayoutMode::Pageless`]: single infinite canvas, document-width content.
//! - [`LayoutMode::Reflow`]: single infinite canvas, caller-supplied width.
//!
//! # Output
//!
//! Layout produces a [`DocumentLayout`] containing [`PositionedItem`]s, each
//! carrying absolute coordinates ready for a renderer such as `loki-vello`.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod color;
pub mod error;
pub mod flow;
pub mod font;
pub mod font_handle;
/// Whether Parley snaps line metrics to the pixel grid while laying text out.
///
/// **`false`, and deliberately so.** Parley's `quantize` aligns layout
/// coordinates to `1/scale` units, which is "the easiest way to avoid blurry
/// text" only when the layout is built at the scale it will be painted at.
/// Loki's is not: layout is computed in **points** (`display_scale` is `1.0` on
/// the print, export and conformance paths) and painted later at an arbitrary
/// zoom × DPI. Quantising there rounds every baseline to a whole point — two
/// device pixels at the 144 dpi the goldens compare at — which is far coarser
/// than the grid it is supposed to protect.
///
/// Parley's own guidance for this case is to pass `false` and quantise just
/// before painting, which is what the renderers do (see
/// [`items::clip_bottom_device_px`] for the clip edge; glyph baselines are
/// snapped by the rasteriser).
///
/// Measured against Word's own PDF for `iris-blueprint.docx`: switching it off
/// improved **all 14 pages** — mean SSIM rose on every one, failing 64×64
/// regions fell 511 → 453, and page 9 became clean.
pub(crate) const QUANTIZE_LAYOUT: bool = false;

pub mod geometry;
#[path = "hatch.rs"]
pub mod hatch;
pub mod incremental;
pub mod items;
mod layout_entry;
mod list_marker;
mod math;
pub mod measure;
pub mod mode;
mod options;
mod paginate_blanks;
pub mod para;
mod para_band;
mod para_cache;
mod para_drop_cap;
mod para_emit;
pub mod resolve;
pub mod result;
#[path = "revision_filter.rs"]
mod revision_filter;
mod revision_style;
mod table_shading;
pub use color::LayoutColor;
pub use error::{LayoutError, LayoutResult};
pub use flow::{FlowOutput, LayoutWarning, flow_section};
pub use font::FontResources;
pub use font_handle::SharedFontResources;
pub use geometry::{LAYOUT_EPSILON_PT, LayoutInsets, LayoutPoint, LayoutRect, LayoutSize};
pub use hatch::{HatchPattern, HatchSegment, PositionedHatch};
pub use incremental::diff::{block_comparisons, reset_block_comparisons};
pub use incremental::{
    FlowCheckpoint, PageStart, PaginatedReuse, document_has_notes, relayout_paginated_incremental,
};
pub use items::{
    BorderEdge, BorderStyle, DecorationKind, GlyphEntry, GlyphSynthesis, PositionedBorderRect,
    PositionedDecoration, PositionedGlyphRun, PositionedImage, PositionedItem, PositionedRect,
};
pub use layout_entry::{layout_document, layout_paginated_full};
pub use mode::LayoutMode;
pub use options::{FieldContext, LayoutOptions, RevisionDisplay, SpellState};
pub use para::{
    Affinity, ByteIndexMap, CursorRect, HitTestResult, ParagraphLayout, ResolvedLineHeight,
    ResolvedParaProps, StyleSpan, layout_paragraph,
};
pub use resolve::{
    CollectedImage, CollectedNote, emu_to_pt, flatten_paragraph, pts_to_f32, resolve_char_props,
    resolve_color, resolve_para_props,
};
pub use result::{
    CellRotation, ContinuousLayout, DocumentLayout, LayoutPage, PageEditingData, PageParagraphData,
    PaginatedLayout,
};

/// Minimum table row height in points.
pub const MIN_ROW_HEIGHT: f32 = 0.0;

/// Total width (points) reserved to the right of the page for the comment
/// gutter panel (gap + card width). Hosts widen the scrollable/canvas area by
/// this much when a paginated layout contains comment items, so the panel is
/// reachable. See [`result::LayoutPage::comment_items`].
pub const COMMENT_GUTTER_WIDTH: f32 = 192.0;

#[cfg(test)]
#[path = "flow_spell_break_tests.rs"]
mod flow_spell_break_tests;

#[cfg(test)]
#[path = "flow_spell_condition_tests.rs"]
mod flow_spell_condition_tests;
