// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Rotated table-cell editing geometry: the [`CellRotation`] affine (mirroring
//! the paint-time `RotatedGroup` transform) and the [`super::PageParagraphData`]
//! page↔paragraph-local mapping helpers that invert it for hit-testing and
//! caret placement. Split out of `result.rs` (Phase 7.1 / feature 4b.5).

use super::PageParagraphData;

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
}
