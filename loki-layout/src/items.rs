// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Renderer-agnostic positioned draw items.
//!
//! [`PositionedItem`] is the central output type of `loki-layout`. Each
//! variant describes a single draw command with an absolute position in
//! layout space. The `loki-vello` crate translates these into Vello scene
//! commands; `loki-layout` itself has no Vello or GPU types.

use std::sync::Arc;

use crate::color::LayoutColor;
use crate::geometry::{LayoutPoint, LayoutRect};

/// A single renderer-agnostic draw item with an absolute position in layout
/// space.
///
/// The `loki-vello` crate translates these into Vello scene commands.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum PositionedItem {
    /// A run of shaped glyphs from Parley.
    GlyphRun(PositionedGlyphRun),
    /// A filled rectangle (backgrounds, table cell fills, etc.).
    FilledRect(PositionedRect),
    /// A border rectangle (stroked, not filled).
    BorderRect(PositionedBorderRect),
    /// An image at a position.
    Image(PositionedImage),
    /// A text decoration (underline, strikethrough, overline).
    Decoration(PositionedDecoration),
    /// A horizontal rule.
    HorizontalRule(PositionedRect),
    /// A group of items rendered inside a Vello clip layer.
    ///
    /// `clip_rect` is in page-content-area-local coordinates (same space as
    /// the child items' origins). Used to render paragraph fragments that
    /// span a page boundary: each fragment carries all paragraph items but
    /// the clip rect masks lines belonging to the other page.
    ClippedGroup {
        /// Clip rectangle in page-content-area coordinates.
        clip_rect: LayoutRect,
        /// Items to render inside the clip.
        items: Vec<PositionedItem>,
    },
}

impl PositionedItem {
    /// Applies a translation to the item's coordinates.
    pub fn translate(&mut self, dx: f32, dy: f32) {
        match self {
            Self::GlyphRun(r) => {
                r.origin.x += dx;
                r.origin.y += dy;
            }
            Self::FilledRect(r) | Self::HorizontalRule(r) => {
                r.rect.origin.x += dx;
                r.rect.origin.y += dy;
            }
            Self::BorderRect(r) => {
                r.rect.origin.x += dx;
                r.rect.origin.y += dy;
            }
            Self::Image(r) => {
                r.rect.origin.x += dx;
                r.rect.origin.y += dy;
            }
            Self::Decoration(d) => {
                d.x += dx;
                d.y += dy;
            }
            Self::ClippedGroup { clip_rect, items } => {
                clip_rect.origin.x += dx;
                clip_rect.origin.y += dy;
                for item in items {
                    item.translate(dx, dy);
                }
            }
        }
    }
}

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

/// A filled rectangle with a solid color.
#[derive(Debug, Clone)]
pub struct PositionedRect {
    /// Position and dimensions.
    pub rect: LayoutRect,
    /// Fill color.
    pub color: LayoutColor,
}

/// A border rectangle with independently styled edges.
#[derive(Debug, Clone)]
pub struct PositionedBorderRect {
    /// Position and dimensions.
    pub rect: LayoutRect,
    /// Top edge, or `None` if absent.
    pub top: Option<BorderEdge>,
    /// Right edge, or `None` if absent.
    pub right: Option<BorderEdge>,
    /// Bottom edge, or `None` if absent.
    pub bottom: Option<BorderEdge>,
    /// Left edge, or `None` if absent.
    pub left: Option<BorderEdge>,
}

/// A single border edge with color, width, and style.
#[derive(Debug, Clone, Copy)]
pub struct BorderEdge {
    /// Border color.
    pub color: LayoutColor,
    /// Border width in points.
    pub width: f32,
    /// Border style (solid, dashed, etc.).
    pub style: BorderStyle,
}

/// The stroke pattern for a border edge.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    /// Continuous line.
    Solid,
    /// Evenly spaced dashes.
    Dashed,
    /// Evenly spaced dots.
    Dotted,
    /// Two parallel lines.
    Double,
}

/// An image positioned in layout space.
#[derive(Debug, Clone)]
pub struct PositionedImage {
    /// Position and dimensions.
    pub rect: LayoutRect,
    /// Data URI or external URL; resolved by the renderer.
    pub src: String,
    /// Alternate text for accessibility, if present.
    pub alt: Option<String>,
}

/// A text decoration line (underline, strikethrough, or overline).
#[derive(Debug, Clone)]
pub struct PositionedDecoration {
    /// Start x of the decoration in layout space.
    pub x: f32,
    /// Baseline y in layout space.
    pub y: f32,
    /// Width of the decoration line in points.
    pub width: f32,
    /// Thickness of the line in points.
    pub thickness: f32,
    /// The decoration type.
    pub kind: DecorationKind,
    /// Line color.
    pub color: LayoutColor,
}

/// The kind of text decoration.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationKind {
    /// Line drawn below the text baseline.
    Underline,
    /// Line drawn through the middle of the text.
    Strikethrough,
    /// Line drawn above the text.
    Overline,
}
