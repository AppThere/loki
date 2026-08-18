// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Renderer-agnostic positioned draw items.
//!
//! [`PositionedItem`] is the central output type of `loki-layout`. Each
//! variant describes a single draw command with an absolute position in
//! layout space. The `loki-vello` crate translates these into Vello scene
//! commands; `loki-layout` itself has no Vello or GPU types.

use crate::color::LayoutColor;
use crate::geometry::{LayoutPoint, LayoutRect};
use crate::hatch::PositionedHatch;

#[path = "items_clip.rs"]
mod clip;
#[path = "items_glyph.rs"]
mod glyph;

pub use clip::{FRAGMENT_CLIP_FLOOR_SLACK_PT, clip_bottom_device_px};
pub use glyph::{GlyphEntry, GlyphSynthesis, PositionedGlyphRun};

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
    /// A hatch-shaded rectangle (`w:shd` line/cross texture): an optional
    /// background fill overlaid with hatch lines. The renderer draws the lines;
    /// see [`PositionedHatch`].
    HatchRect(PositionedHatch),
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
    /// span a page boundary: each fragment carries the items near its own
    /// y-range (`ParagraphLayout::items_in_y_range`, feature 6.3) and the
    /// clip rect masks anything belonging to the other page.
    ClippedGroup {
        /// Clip rectangle in page-content-area coordinates.
        clip_rect: LayoutRect,
        /// Items to render inside the clip.
        items: Vec<PositionedItem>,
    },
    /// A group of items rendered with a rotation transform applied.
    /// Used for rotated table cell content (CellTextDirection != LrTb).
    RotatedGroup {
        /// Origin (top-left) of the rotated cell in physical layout space.
        origin: LayoutPoint,
        /// Rotation in degrees clockwise (90, 180, 270).
        degrees: f32,
        /// Width of the original (pre-rotation) content area.
        content_width: f32,
        /// Height of the original (pre-rotation) content area.
        content_height: f32,
        /// Items to render inside the rotation transform.
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
            Self::HatchRect(h) => {
                h.rect.origin.x += dx;
                h.rect.origin.y += dy;
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
            Self::RotatedGroup { origin, .. } => {
                origin.x += dx;
                origin.y += dy;
            }
        }
    }

    /// Releases spare capacity in this item and anything nested inside it.
    ///
    /// Glyph runs are built by `push`, so `glyphs` carries up to 2× doubling
    /// slack. That slack used to be discarded for free: the shaping cache was
    /// populated with `result.clone()`, and cloning a `Vec` allocates exactly
    /// `len`, so the *cached* copy was compact and the loose original was
    /// transient. Sharing one allocation with the editing index (Spec 09 S9-1)
    /// removed that clone and with it the accidental compaction — worth ~11
    /// B/char of long-lived residency, which the E0 sweep caught as a rise in
    /// the read-only condition after an otherwise clean win.
    ///
    /// Made explicit here rather than left to a clone, so the compaction has an
    /// owner and survives the next refactor that removes a copy.
    pub(crate) fn shrink_to_fit(&mut self) {
        match self {
            Self::GlyphRun(r) => r.glyphs.shrink_to_fit(),
            Self::ClippedGroup { items, .. } | Self::RotatedGroup { items, .. } => {
                items.shrink_to_fit();
                for item in items {
                    item.shrink_to_fit();
                }
            }
            Self::FilledRect(_)
            | Self::HorizontalRule(_)
            | Self::HatchRect(_)
            | Self::BorderRect(_)
            | Self::Image(_)
            | Self::Decoration(_) => {}
        }
    }
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
    /// Gap in points between the border and the text it encloses — OOXML
    /// `w:pBdr/*@w:space`, ODF `fo:padding-*`.
    ///
    /// The rule sits this far outside the text, so together with
    /// [`width`](Self::width) it is room the paragraph occupies *in addition
    /// to* its lines: Word's blank-paragraph-with-a-bottom-border idiom (a
    /// horizontal rule) is `space + width` tall even though it has no text.
    pub spacing: f32,
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
    /// The decoration type (which band the line sits in).
    pub kind: DecorationKind,
    /// How the line is drawn (solid / double / dotted / dashed / wave / thick).
    pub style: DecorationStyle,
    /// Line color.
    pub color: LayoutColor,
}

/// How a decoration line is stroked. Orthogonal to [`DecorationKind`] (which
/// says *where* the line sits): a `w:u`/`style:text-underline-style` value maps
/// to one of these for underlines, and `w:strike`/`w:dstrike` to `Solid`/
/// `Double` for strikethroughs. Spelling squiggles are always [`Self::Wave`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DecorationStyle {
    /// A single solid line (the default).
    #[default]
    Solid,
    /// Two parallel solid lines.
    Double,
    /// A row of dots.
    Dotted,
    /// A row of short dashes.
    Dashed,
    /// A sine-like wave (also used for spelling squiggles).
    Wave,
    /// A single line at roughly double thickness.
    Thick,
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
    /// Wavy line drawn below the text to mark a spelling error.
    ///
    /// Carried by the layout like an underline (it sits in the same below-text
    /// band), but the renderer paints it as a wave rather than a straight
    /// stroke. Emitted from spell-check results, not from character styling, so
    /// it never round-trips to a document format.
    Spelling,
}
