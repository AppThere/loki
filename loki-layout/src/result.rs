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
            Self::Paginated(p) => Box::new(p.pages.iter().flat_map(LayoutPage::all_items))
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
    pub pages: Vec<LayoutPage>,
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
mod tests {
    use super::*;
    use crate::color::LayoutColor;
    use crate::geometry::LayoutRect;
    use crate::items::PositionedRect;

    fn make_filled(x: f32) -> PositionedItem {
        PositionedItem::FilledRect(PositionedRect {
            rect: LayoutRect::new(x, 0.0, 10.0, 10.0),
            color: LayoutColor::BLACK,
        })
    }

    #[test]
    fn continuous_all_items_count() {
        let layout = DocumentLayout::Continuous(ContinuousLayout {
            content_width: 500.0,
            total_height: 200.0,
            items: vec![make_filled(0.0), make_filled(20.0), make_filled(40.0)],
            paragraphs: vec![],
        });
        assert_eq!(layout.all_items().count(), 3);
    }

    fn para(text: &str, block_index: usize, origin: (f32, f32)) -> PageParagraphData {
        use crate::font::FontResources;
        use crate::para::{ResolvedParaProps, StyleSpan, layout_paragraph};
        let mut resources = FontResources::new();
        let layout = layout_paragraph(
            &mut resources,
            text,
            &[StyleSpan {
                range: 0..text.len(),
                font_name: None,
                font_size: 12.0,
                bold: false,
                italic: false,
                color: LayoutColor::BLACK,
                underline: None,
                strikethrough: None,
                line_height: None,
                vertical_align: None,
                highlight_color: None,
                letter_spacing: None,
                font_variant: None,
                word_spacing: None,
                shadow: false,
                link_url: None,
            }],
            &ResolvedParaProps::default(),
            400.0,
            1.0,
            true,
        );
        PageParagraphData {
            block_index,
            layout: Arc::new(layout),
            origin,
        }
    }

    fn two_para_continuous() -> ContinuousLayout {
        let p0 = para("Hello world", 0, (0.0, 0.0));
        let h0 = p0.layout.height;
        let p1 = para("Second line here", 1, (0.0, h0));
        ContinuousLayout {
            content_width: 400.0,
            total_height: h0 + p1.layout.height,
            items: vec![],
            paragraphs: vec![p0, p1],
        }
    }

    #[test]
    fn selection_rects_collapsed_is_empty() {
        let cl = two_para_continuous();
        assert!(cl.selection_rects((0, 3), (0, 3)).is_empty());
    }

    #[test]
    fn selection_rects_within_paragraph() {
        let cl = two_para_continuous();
        let rects = cl.selection_rects((0, 0), (0, 5));
        assert!(!rects.is_empty(), "expected highlight rects");
        // Confined to the first paragraph (origin y = 0, near the top).
        assert!(rects.iter().all(|r| r.origin.y < 30.0));
    }

    #[test]
    fn selection_rects_span_two_paragraphs() {
        let cl = two_para_continuous();
        // Split at the boundary midpoint; line ascent puts a rect's top a point
        // or so above the nominal paragraph origin, so an exact `>= origin`
        // comparison is too strict.
        let mid = cl.paragraphs[1].origin.1 / 2.0;
        let rects = cl.selection_rects((0, 6), (1, 6));
        // Endpoint order is normalised, so reversing gives the same result.
        let rev = cl.selection_rects((1, 6), (0, 6));
        assert_eq!(rects.len(), rev.len());
        assert!(rects.iter().any(|r| r.origin.y < mid)); // first paragraph
        assert!(rects.iter().any(|r| r.origin.y > mid)); // second paragraph
    }

    #[test]
    fn hit_test_and_cursor_round_trip() {
        let cl = two_para_continuous();
        // A click on the second paragraph resolves to block 1.
        let (block, _byte) = cl
            .hit_test(2.0, cl.paragraphs[1].origin.1 + 2.0)
            .expect("hit");
        assert_eq!(block, 1);
        // Caret for the second paragraph sits at/after its canvas origin.
        let cr = cl.cursor_rect_canvas(1, 0).expect("caret");
        assert!(cr.y >= cl.paragraphs[1].origin.1 - 1.0);
    }

    #[test]
    fn paginated_all_items_across_pages() {
        let page1 = LayoutPage {
            page_number: 1,
            page_size: LayoutSize::new(595.0, 842.0),
            margins: LayoutInsets::uniform(72.0),
            content_items: vec![make_filled(0.0), make_filled(10.0)],
            header_items: vec![make_filled(20.0)],
            footer_items: vec![],
            header_height: 0.0,
            footer_height: 0.0,
            editing_data: None,
        };
        let page2 = LayoutPage {
            page_number: 2,
            page_size: LayoutSize::new(595.0, 842.0),
            margins: LayoutInsets::uniform(72.0),
            content_items: vec![make_filled(0.0)],
            header_items: vec![],
            footer_items: vec![make_filled(30.0)],
            header_height: 0.0,
            footer_height: 0.0,
            editing_data: None,
        };
        let layout = DocumentLayout::Paginated(PaginatedLayout {
            page_size: LayoutSize::new(595.0, 842.0),
            pages: vec![page1, page2],
        });
        // page1: 2 content + 1 header = 3; page2: 1 content + 1 footer = 2 → total 5
        assert_eq!(layout.all_items().count(), 5);
    }

    #[test]
    fn layout_page_content_rect() {
        let page = LayoutPage {
            page_number: 1,
            page_size: LayoutSize::new(595.0, 842.0),
            margins: LayoutInsets {
                top: 72.0,
                right: 72.0,
                bottom: 72.0,
                left: 72.0,
            },
            content_items: vec![],
            header_items: vec![],
            footer_items: vec![],
            header_height: 0.0,
            footer_height: 0.0,
            editing_data: None,
        };
        let cr = page.content_rect();
        assert_eq!(cr.x(), 72.0);
        assert_eq!(cr.y(), 72.0);
        assert_eq!(cr.width(), 595.0 - 144.0);
        assert_eq!(cr.height(), 842.0 - 144.0);
    }

    #[test]
    fn document_layout_total_height_paginated() {
        let make_page = |n: usize| LayoutPage {
            page_number: n,
            page_size: LayoutSize::new(595.0, 842.0),
            margins: LayoutInsets::uniform(72.0),
            content_items: vec![],
            header_items: vec![],
            footer_items: vec![],
            header_height: 0.0,
            footer_height: 0.0,
            editing_data: None,
        };
        let layout = DocumentLayout::Paginated(PaginatedLayout {
            page_size: LayoutSize::new(595.0, 842.0),
            pages: vec![make_page(1), make_page(2)],
        });
        assert_eq!(layout.total_height(), 842.0 * 2.0);
    }

    #[test]
    fn document_layout_content_width_continuous() {
        let layout = DocumentLayout::Continuous(ContinuousLayout {
            content_width: 480.0,
            total_height: 100.0,
            items: vec![],
            paragraphs: vec![],
        });
        assert_eq!(layout.content_width(), 480.0);
    }
}
