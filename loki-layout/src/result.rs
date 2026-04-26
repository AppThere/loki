// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Top-level layout output types.
//!
//! [`DocumentLayout`] is the root result produced by the layout engine.
//! Depending on the [`crate::LayoutMode`] used, it is either a
//! [`PaginatedLayout`] (multiple fixed-size pages) or a [`ContinuousLayout`]
//! (a single infinite canvas).

use std::sync::Arc;

use crate::geometry::{LayoutInsets, LayoutRect, LayoutSize};
use crate::items::PositionedItem;
use crate::para::ParagraphLayout;

/// Per-page editing data that maps page-local coordinates to paragraph layouts.
///
/// Only populated when `LayoutOptions::preserve_for_editing` is `true`.
/// Indexed in the same order as `LayoutPage::content_items`.
#[derive(Debug, Clone)]
pub struct PageEditingData {
    /// `Arc`-wrapped `ParagraphLayout` for each content item on the page,
    /// in the same order as `LayoutPage::content_items`.
    ///
    /// An entry is `None` when the corresponding content item is not a
    /// paragraph (e.g. a table cell placeholder from a prior pipeline stage).
    pub paragraph_layouts: Vec<Option<Arc<ParagraphLayout>>>,
    /// Page-local `(x, y)` origin of each paragraph, in points.
    ///
    /// Used by the editing layer to translate a page-local pointer position
    /// into paragraph-local coordinates before calling `hit_test_point`.
    pub paragraph_origins: Vec<(f32, f32)>,
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
            Self::Paginated(p) => Box::new(
                p.pages.iter().flat_map(LayoutPage::all_items),
            ) as Box<dyn Iterator<Item = &PositionedItem> + '_>,
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
        });
        assert_eq!(layout.all_items().count(), 3);
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
            margins: LayoutInsets { top: 72.0, right: 72.0, bottom: 72.0, left: 72.0 },
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
        });
        assert_eq!(layout.content_width(), 480.0);
    }
}
