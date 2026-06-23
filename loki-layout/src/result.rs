// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Top-level layout output types.
//!
//! [`DocumentLayout`] is the root result produced by the layout engine.
//! Depending on the [`crate::LayoutMode`] used, it is either a
//! [`PaginatedLayout`] (multiple fixed-size pages) or a [`ContinuousLayout`]
//! (a single infinite canvas).

use std::sync::Arc;

use crate::geometry::{LayoutInsets, LayoutRect, LayoutSize};
use crate::items::PositionedItem;
use crate::para::{CursorRect, ParagraphLayout};

/// Per-page editing data that maps page-local coordinates to paragraph layouts.
///
/// Only populated when `LayoutOptions::preserve_for_editing` is `true`.
#[derive(Debug, Clone)]
pub struct PageEditingData {
    /// Data for each paragraph that appears (even partially) on this page.
    pub paragraphs: Vec<PageParagraphData>,
}

/// Metadata for a single paragraph fragment on a page.
#[derive(Debug, Clone)]
pub struct PageParagraphData {
    /// Global index of the paragraph block in the document.
    pub block_index: usize,
    /// The preserved layout data for hit-testing and cursor positioning.
    pub layout: Arc<ParagraphLayout>,
    /// Page-local `(x, y)` origin of the paragraph in points, relative to
    /// the page content area (i.e. after margins).
    pub origin: (f32, f32),
}

/// The result of laying out a document.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum DocumentLayout {
    /// Output of [`crate::LayoutMode::Paginated`].
    Paginated(PaginatedLayout),
    /// Output of [`crate::LayoutMode::Pageless`] or [`crate::LayoutMode::Reflow`].
    Continuous(ContinuousLayout),
}

impl DocumentLayout {
    /// All positioned items across all pages or the whole canvas.
    ///
    /// Useful for testing without caring about page structure.
    pub fn all_items(&self) -> impl Iterator<Item = &PositionedItem> + '_ {
        match self {
            Self::Paginated(p) => Box::new(p.pages.iter().flat_map(|pg| pg.all_items()))
                as Box<dyn Iterator<Item = &PositionedItem> + '_>,
            Self::Continuous(c) => Box::new(c.items.iter()),
        }
    }

    /// Total document height in points.
    ///
    /// For paginated layouts this is `page_height × page_count`.
    pub fn total_height(&self) -> f32 {
        match self {
            Self::Paginated(p) => p.page_size.height * p.pages.len() as f32,
            Self::Continuous(c) => c.total_height,
        }
    }

    /// Content width used for layout in points.
    pub fn content_width(&self) -> f32 {
        match self {
            Self::Paginated(p) => p.content_width(),
            Self::Continuous(c) => c.content_width,
        }
    }
}

/// Layout result for paginated mode.
#[derive(Debug, Clone)]
pub struct PaginatedLayout {
    /// Physical page dimensions.
    pub page_size: LayoutSize,
    /// All pages in document order.
    ///
    /// Pages are `Arc`-wrapped so incremental relayout
    /// ([`crate::relayout_paginated_incremental`]) can reuse an unchanged page
    /// by cloning its `Arc` (a refcount bump) instead of deep-copying its
    /// content — the key to O(changed) per-keystroke relayout.
    pub pages: Vec<Arc<LayoutPage>>,
}

impl PaginatedLayout {
    /// Content width in points (page width minus the first page's horizontal
    /// margins, or just page width if there are no pages).
    pub fn content_width(&self) -> f32 {
        self.pages
            .first()
            .map(|p| self.page_size.width - p.margins.horizontal())
            .unwrap_or(self.page_size.width)
    }
}

/// A single page in a paginated layout.
#[derive(Debug, Clone)]
pub struct LayoutPage {
    /// 1-indexed page number.
    pub page_number: usize,
    /// Physical dimensions of this page.
    pub page_size: LayoutSize,
    /// Margins applied to this page.
    pub margins: LayoutInsets,
    /// Items in the content area. Origins are content-area-local: `(0, 0)` is
    /// the content-area top-left (i.e. page origin offset by `margins`).
    /// The painter is responsible for adding the margin offset at render time.
    pub content_items: Vec<PositionedItem>,
    /// Items in the header area. Origins are page-local (top-left of page).
    pub header_items: Vec<PositionedItem>,
    /// Items in the footer area. Origins are page-local (top-left of page).
    pub footer_items: Vec<PositionedItem>,
    /// Comment-panel items rendered in the gutter to the right of the page.
    /// Origins are page-local; their x extends past `page_size.width`. Empty
    /// when the page has no anchored comments.
    pub comment_items: Vec<PositionedItem>,
    /// Rendered height of the header content in points (0.0 if no header).
    pub header_height: f32,
    /// Rendered height of the footer content in points (0.0 if no footer).
    pub footer_height: f32,
    /// Paragraph layout data for hit testing and cursor positioning.
    ///
    /// `None` when `LayoutOptions::preserve_for_editing` was `false`
    /// (read-only mode). When `Some`, each entry corresponds to a paragraph
    /// placed on this page, in the same order they were flowed.
    pub editing_data: Option<PageEditingData>,
}

impl LayoutPage {
    /// Content area rectangle: the page rect inset by margins.
    pub fn content_rect(&self) -> LayoutRect {
        LayoutRect::new(
            self.margins.left,
            self.margins.top,
            self.page_size.width - self.margins.horizontal(),
            self.page_size.height - self.margins.vertical(),
        )
    }

    /// All items on this page (content + header + footer).
    pub fn all_items(&self) -> impl Iterator<Item = &PositionedItem> + '_ {
        self.content_items
            .iter()
            .chain(self.header_items.iter())
            .chain(self.footer_items.iter())
    }
}

/// Layout result for continuous (pageless / reflow) mode.
#[derive(Debug, Clone)]
pub struct ContinuousLayout {
    /// The width used for layout in points.
    ///
    /// Either the document content width (pageless) or the caller-supplied
    /// reflow width.
    pub content_width: f32,
    /// Total height of all content in points.
    pub total_height: f32,
    /// All positioned items. Origins are absolute within the canvas.
    pub items: Vec<PositionedItem>,
    /// Per-paragraph editing data (layout + absolute canvas origin), in document
    /// order. Populated only when `LayoutOptions::preserve_for_editing` is
    /// `true`; empty otherwise. Used for hit-testing and cursor positioning in
    /// the reflow editor, mirroring [`PageEditingData`] for paginated pages.
    pub paragraphs: Vec<PageParagraphData>,
}

impl ContinuousLayout {
    /// Find the editing paragraph for `block_index`.
    pub fn paragraph(&self, block_index: usize) -> Option<&PageParagraphData> {
        self.paragraphs
            .iter()
            .find(|p| p.block_index == block_index)
    }

    /// Hit-test a point in canvas coordinates (layout points), returning
    /// `(block_index, byte_offset)`.  Mirrors `hit_test_page` for the
    /// continuous canvas: the covering paragraph, else the first (above all
    /// content) or last (below it).
    pub fn hit_test(&self, canvas_x: f32, canvas_y: f32) -> Option<(usize, usize)> {
        if self.paragraphs.is_empty() {
            return None;
        }
        let para = self
            .paragraphs
            .iter()
            .rev()
            .find(|p| p.origin.1 <= canvas_y && canvas_y <= p.origin.1 + p.layout.height)
            .or_else(|| {
                if canvas_y < self.paragraphs[0].origin.1 {
                    self.paragraphs.first()
                } else {
                    self.paragraphs.last()
                }
            })?;
        let x_in = canvas_x - para.origin.0;
        let y_in = (canvas_y - para.origin.1).max(0.0);
        let byte = para
            .layout
            .hit_test_point(x_in, y_in)
            .map_or(0, |h| h.byte_offset);
        Some((para.block_index, byte))
    }

    /// Caret rectangle in canvas coordinates for `(block_index, byte_offset)`.
    pub fn cursor_rect_canvas(&self, block_index: usize, byte_offset: usize) -> Option<CursorRect> {
        let para = self.paragraph(block_index)?;
        let cr = para.layout.cursor_rect(byte_offset)?;
        Some(CursorRect {
            x: para.origin.0 + cr.x,
            y: para.origin.1 + cr.y,
            height: cr.height,
        })
    }

    /// Selection highlight rectangles in canvas coordinates between two document
    /// positions `(block_index, byte_offset)`.  Whole intermediate paragraphs
    /// are spanned (a byte offset of `usize::MAX` clamps to the paragraph end).
    /// Empty when the two positions are equal.
    pub fn selection_rects(&self, a: (usize, usize), b: (usize, usize)) -> Vec<LayoutRect> {
        // Order the endpoints by document position (paragraph order, then byte).
        let pos = |bi: usize| self.paragraphs.iter().position(|p| p.block_index == bi);
        let (Some(pa), Some(pb)) = (pos(a.0), pos(b.0)) else {
            return Vec::new();
        };
        let ((start_i, start), (end_i, end)) = if (pa, a.1) <= (pb, b.1) {
            ((pa, a), (pb, b))
        } else {
            ((pb, b), (pa, a))
        };
        if start == end {
            return Vec::new();
        }

        let mut rects = Vec::new();
        for i in start_i..=end_i {
            let para = &self.paragraphs[i];
            let lo = if i == start_i { start.1 } else { 0 };
            // Whole paragraph to the end for all but the final one.
            let hi = if i == end_i { end.1 } else { usize::MAX };
            for mut r in para.layout.selection_rects(lo, hi) {
                r.origin.x += para.origin.0;
                r.origin.y += para.origin.1;
                rects.push(r);
            }
        }
        rects
    }
}

#[cfg(test)]
#[path = "result_tests.rs"]
mod tests;
