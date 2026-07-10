// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Even/odd section-break blank-page insertion (feature 5.10).
//!
//! An `evenPage` / `oddPage` section break (OOXML `w:sectPr/w:type`) starts the
//! section on the next even / odd page. When the section would otherwise begin
//! on the wrong parity, a single blank filler page is inserted before it —
//! matching Word. Used by [`crate::layout_paginated_full`].

use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::layout::section::SectionStart;

use crate::geometry::{LayoutInsets, LayoutSize};
use crate::resolve::pts_to_f32;
use crate::result::LayoutPage;

/// Whether a blank filler page must precede a section that starts with the given
/// [`SectionStart`], given the count of pages already emitted.
///
/// Page numbers are 1-based: the section would start on page `page_count + 1`.
/// The document's first section (`page_count == 0`) never gets a filler — its
/// break type is immaterial (it starts the document on page 1).
pub(crate) fn needs_blank_before(start: SectionStart, page_count: usize) -> bool {
    if page_count == 0 {
        return false;
    }
    let next = page_count + 1;
    match start {
        SectionStart::EvenPage => !next.is_multiple_of(2), // wants even, would be odd
        SectionStart::OddPage => next.is_multiple_of(2),   // wants odd, would be even
        _ => false,
    }
}

/// A blank filler page carrying `pl`'s geometry (size + margins) and no content,
/// header, footer, or editing data.
pub(crate) fn blank_page(page_number: usize, pl: &PageLayout) -> LayoutPage {
    LayoutPage {
        page_number,
        page_size: LayoutSize::new(
            pts_to_f32(pl.page_size.width),
            pts_to_f32(pl.page_size.height),
        ),
        margins: LayoutInsets {
            top: pts_to_f32(pl.margins.top),
            right: pts_to_f32(pl.margins.right),
            bottom: pts_to_f32(pl.margins.bottom),
            left: pts_to_f32(pl.margins.left),
        },
        content_items: Vec::new(),
        header_items: Vec::new(),
        footer_items: Vec::new(),
        comment_items: Vec::new(),
        header_height: 0.0,
        footer_height: 0.0,
        editing_data: None,
    }
}

#[cfg(test)]
#[path = "paginate_blanks_tests.rs"]
mod tests;
