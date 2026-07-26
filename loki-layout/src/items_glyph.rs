// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shaped-glyph output types: [`PositionedGlyphRun`] and its parts.
//!
//! Extracted from `items.rs` for the 300-line ceiling. Re-exported from the
//! parent, so `crate::items::PositionedGlyphRun` and every public path through
//! `lib.rs` are unchanged.
//!
//! `glyphs` is the largest per-paragraph allocation in a layout, and the one
//! Spec 09 S9-1 made shared rather than copied per placement; see
//! [`super::PositionedItem::shrink_to_fit`] for why its spare capacity is now
//! released explicitly.

use std::sync::Arc;

use crate::color::LayoutColor;
use crate::geometry::LayoutPoint;

/// A positioned and shaped glyph run ready for rendering.
#[derive(Debug, Clone)]
pub struct PositionedGlyphRun {
    /// Top-left origin of the run in layout space.
    pub origin: LayoutPoint,
    /// Raw font table data for the face used in this run.
    ///
    /// Kept as raw bytes to avoid `loki-layout` depending on Parley's glyph
    /// types at the output level. `loki-vello` decodes this using the same
    /// Parley version.
    pub font_data: Arc<Vec<u8>>,
    /// Font index within the font data (for TTC / font collections).
    pub font_index: u32,
    /// Font size in points.
    pub font_size: f32,
    /// Individual glyphs in this run.
    pub glyphs: Vec<GlyphEntry>,
    /// Text color.
    pub color: LayoutColor,
    /// Synthesis flags (bold/italic synthesis).
    pub synthesis: GlyphSynthesis,
    /// Normalized variation coordinates (F2Dot14 raw i16, one per fvar axis)
    /// for this run's selected face, as resolved by Parley. Non-empty only for
    /// variable fonts — e.g. the bundled Arimo (Arial substitute) is a `wght`
    /// variable font, so a bold run carries its `wght=700` coordinate here.
    /// Both painters must apply these; rendering the default (all-zero) master
    /// instead paints regular-weight glyphs with bold advances (gap: bold Arial
    /// looked "wide but not bold").
    pub normalized_coords: Vec<i16>,
    /// Hyperlink URL if this run is part of a link. `None` for non-link text.
    ///
    /// A blue-tint underlay hint is rendered by `loki-vello`, a point resolves
    /// to its URL via `ContinuousLayout::link_at` / `PageEditingData::link_at`,
    /// and Ctrl/Cmd+click opens it in both paginated and reflow modes
    /// (feature 5.11).
    pub link_url: Option<String>,
}

/// A single glyph with its position relative to the run origin.
#[derive(Debug, Clone, Copy)]
pub struct GlyphEntry {
    /// Glyph ID.
    pub id: u16,
    /// X position relative to the run origin.
    pub x: f32,
    /// Y position relative to the run origin (baseline offset).
    pub y: f32,
    /// Horizontal advance in points.
    pub advance: f32,
}

/// Font synthesis flags applied when the requested style is not available.
#[derive(Debug, Clone, Copy, Default)]
pub struct GlyphSynthesis {
    /// Bold synthesis is active.
    pub bold: bool,
    /// Italic synthesis is active.
    pub italic: bool,
}
