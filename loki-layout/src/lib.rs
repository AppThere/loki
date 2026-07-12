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
pub mod geometry;
pub mod incremental;
pub mod items;
mod layout_entry;
mod list_marker;
mod math;
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
mod revision_style;
mod table_shading;
pub use color::LayoutColor;
pub use error::{LayoutError, LayoutResult};
pub use flow::{FlowOutput, LayoutWarning, flow_section};
pub use font::FontResources;
pub use geometry::{LayoutInsets, LayoutPoint, LayoutRect, LayoutSize};
pub use incremental::{
    FlowCheckpoint, PageStart, PaginatedReuse, document_has_notes, relayout_paginated_incremental,
};
pub use items::{
    BorderEdge, BorderStyle, DecorationKind, GlyphEntry, GlyphSynthesis, PositionedBorderRect,
    PositionedDecoration, PositionedGlyphRun, PositionedImage, PositionedItem, PositionedRect,
};
pub use layout_entry::{layout_document, layout_paginated_full};
pub use mode::LayoutMode;
pub use options::{FieldContext, LayoutOptions, SpellState};
pub use para::{
    Affinity, CursorRect, HitTestResult, ParagraphLayout, ResolvedLineHeight, ResolvedParaProps,
    StyleSpan, layout_paragraph,
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
