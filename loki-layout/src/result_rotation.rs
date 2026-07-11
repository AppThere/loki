// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Rotated table-cell editing geometry: the [`CellRotation`] affine (mirroring
//! the paint-time `RotatedGroup` transform) and the [`super::PageParagraphData`]
//! page↔paragraph-local mapping helpers that invert it for hit-testing and
//! caret placement. Split out of `result.rs` (Phase 7.1 / feature 4b.5).

use super::{PageEditingData, PageParagraphData};
use crate::items::{PositionedGlyphRun, PositionedItem};

impl PageEditingData {
    /// The hyperlink URL under a **content-area-local** point (the same frame
    /// the paginated hit-test uses — subtract `page.margins` from a page-local
    /// click first), if it lands on a hyperlinked glyph run on this page
    /// (feature 5.11). `None` over non-link text or empty space.
    pub fn link_at(&self, content_x: f32, content_y: f32) -> Option<&str> {
        self.paragraphs
            .iter()
            .find_map(|p| p.link_at(content_x, content_y))
    }
}

/// The rigid rotation a rotated table cell applies to its content, mirroring
/// the paint-time [`crate::items::PositionedItem::RotatedGroup`] affine so the
/// editor can invert it. When a [`PageParagraphData`] carries this, its
/// [`PageParagraphData::origin`] is expressed in the cell's **content-local**
/// (pre-rotation) frame, and the transform maps that frame to page coordinates:
///
/// `page = pivot_page + Rot(degrees) · (local − pivot_local)`
///
/// (The pivots are the renderer's `cx/cy_local` and `cx/cy_physical`.)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellRotation {
    /// Clockwise rotation in degrees (90 / 270 for vertical cell text).
    pub degrees: f32,
    /// Rotation pivot in the content-local (pre-rotation) frame.
    pub pivot_local: (f32, f32),
    /// Rotation pivot in page coordinates.
    pub pivot_page: (f32, f32),
}

impl CellRotation {
    /// Map a content-local point to page coordinates (forward transform).
    pub fn local_to_page(&self, lx: f32, ly: f32) -> (f32, f32) {
        let (dx, dy) = (lx - self.pivot_local.0, ly - self.pivot_local.1);
        let t = self.degrees.to_radians();
        let (s, c) = t.sin_cos();
        (
            self.pivot_page.0 + dx * c - dy * s,
            self.pivot_page.1 + dx * s + dy * c,
        )
    }

    /// Map a page point to the content-local frame (inverse transform).
    pub fn page_to_local(&self, px: f32, py: f32) -> (f32, f32) {
        let (dx, dy) = (px - self.pivot_page.0, py - self.pivot_page.1);
        let t = (-self.degrees).to_radians();
        let (s, c) = t.sin_cos();
        (
            self.pivot_local.0 + dx * c - dy * s,
            self.pivot_local.1 + dx * s + dy * c,
        )
    }
}

impl PageParagraphData {
    /// Map a page-coordinate point to this paragraph's local (Parley) frame,
    /// inverting the cell rotation when present. Feed the result to
    /// [`ParagraphLayout::hit_test_point`](crate::para::ParagraphLayout::hit_test_point).
    pub fn hit_local(&self, page_x: f32, page_y: f32) -> (f32, f32) {
        match self.rotation {
            None => (page_x - self.origin.0, page_y - self.origin.1),
            Some(rot) => {
                let (lx, ly) = rot.page_to_local(page_x, page_y);
                (lx - self.origin.0, ly - self.origin.1)
            }
        }
    }

    /// Map a paragraph-local point (e.g. a caret rect corner) to page
    /// coordinates, applying the cell rotation when present. The inverse of
    /// [`hit_local`](Self::hit_local).
    pub fn local_to_page(&self, local_x: f32, local_y: f32) -> (f32, f32) {
        match self.rotation {
            None => (self.origin.0 + local_x, self.origin.1 + local_y),
            Some(rot) => rot.local_to_page(self.origin.0 + local_x, self.origin.1 + local_y),
        }
    }

    /// Content-local vertical extent `[top, bottom]` of this paragraph in the
    /// frame its `origin` lives in — used to find the paragraph covering a hit.
    pub fn local_y_span(&self) -> (f32, f32) {
        (self.origin.1, self.origin.1 + self.layout.height)
    }

    /// The hyperlink URL under a page-coordinate point, if the point lands on a
    /// hyperlinked glyph run in this paragraph (feature 5.11). Inverts the cell
    /// rotation like [`hit_local`](Self::hit_local), then tests each run's box.
    /// Returns `None` for a point outside the paragraph or over non-link text.
    pub fn link_at(&self, page_x: f32, page_y: f32) -> Option<&str> {
        let (lx, ly) = self.hit_local(page_x, page_y);
        if !(0.0..=self.layout.height).contains(&ly) {
            return None;
        }
        link_in_items(&self.layout.items, lx, ly)
    }
}

/// Recursively find the hyperlink URL of the first glyph run whose visual box
/// contains the paragraph-local point `(lx, ly)`. The box matches the renderer's
/// link hint (`loki-vello`'s `paint_link_hint`): width = summed glyph advances,
/// vertical extent = `0.8·font_size` above the baseline to `0.2·font_size` below.
/// Recurses into `ClippedGroup`s (exact line-height wraps lines in one).
fn link_in_items(items: &[PositionedItem], lx: f32, ly: f32) -> Option<&str> {
    for item in items {
        match item {
            PositionedItem::GlyphRun(run) => {
                if let Some(url) = link_in_run(run, lx, ly) {
                    return Some(url);
                }
            }
            PositionedItem::ClippedGroup { items, .. } => {
                if let Some(url) = link_in_items(items, lx, ly) {
                    return Some(url);
                }
            }
            _ => {}
        }
    }
    None
}

/// The run's link URL if `(lx, ly)` lands within its hint box, else `None`.
fn link_in_run(run: &PositionedGlyphRun, lx: f32, ly: f32) -> Option<&str> {
    let url = run.link_url.as_deref()?;
    let width: f32 = run.glyphs.iter().map(|g| g.advance).sum();
    if width <= 0.0 {
        return None;
    }
    let x0 = run.origin.x;
    let y0 = run.origin.y - run.font_size * 0.8;
    let y1 = run.origin.y + run.font_size * 0.2;
    (lx >= x0 && lx <= x0 + width && ly >= y0 && ly <= y1).then_some(url)
}
