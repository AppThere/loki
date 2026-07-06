// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Re-deriving a cursor position's `page_index` from the paginated layout
//! (plan 4b.1 / `3b-3`).
//!
//! A [`DocumentPosition`]'s `page_index` must name the page whose editing
//! data contains the caret's paragraph — hit-testing, cursor painting, and
//! navigation all resolve the paragraph through it. Two things silently
//! invalidate it:
//!
//! - **mutations**: a split/merge or typing near a page boundary relays the
//!   document out, and the caret's paragraph can move to a different page;
//! - **split paragraphs**: a paragraph flowing across a page break has an
//!   entry on *every* page it touches (one shared [`ParagraphLayout`] with
//!   shifted origins), so the right page depends on the byte offset's line.
//!
//! [`recompute_page_index`] handles both: it scans the layout for the pages
//! holding the position's `(block_index, path)` entry and, when the
//! paragraph spans several pages, picks the page whose content band contains
//! the byte's line.
//!
//! [`ParagraphLayout`]: loki_layout::ParagraphLayout

use loki_layout::PaginatedLayout;

use super::cursor::DocumentPosition;

#[cfg(test)]
#[path = "page_locate_tests.rs"]
mod tests;

/// Returns `pos` with its `page_index` re-derived from `layout`.
///
/// When the position's paragraph is found on exactly one page, that page
/// wins. When it spans several pages (a split paragraph), the page whose
/// content band contains the byte offset's line-centre wins; if no band
/// matches (degenerate geometry), the first page holding the paragraph is
/// used. When the paragraph is not in the layout at all (e.g. the layout is
/// momentarily stale), `pos` is returned unchanged.
#[must_use]
pub fn recompute_page_index(layout: &PaginatedLayout, pos: &DocumentPosition) -> DocumentPosition {
    let mut first_holder: Option<usize> = None;
    let mut visible: Option<usize> = None;

    for (pi, page) in layout.pages.iter().enumerate() {
        let Some(ed) = page.editing_data.as_ref() else {
            continue;
        };
        let Some(para) = ed
            .paragraphs
            .iter()
            .find(|p| p.block_index == pos.paragraph_index && p.path == pos.path)
        else {
            continue;
        };
        first_holder.get_or_insert(pi);
        if visible.is_none()
            && let Some(rect) = para.layout.cursor_rect(pos.byte_offset)
        {
            // Content-band check: the line's centre, in page-content
            // coordinates (origin is already content-relative).
            let y_center = rect.y + rect.height / 2.0 + para.origin.1;
            let content_h = page.page_size.height - page.margins.top - page.margins.bottom;
            if y_center >= 0.0 && y_center < content_h {
                visible = Some(pi);
            }
        }
        if visible.is_some() {
            break;
        }
    }

    let new_page = match (visible, first_holder) {
        (Some(pi), _) => pi,
        (None, Some(pi)) => pi,
        (None, None) => pos.page_index,
    };
    if new_page == pos.page_index {
        return pos.clone();
    }
    DocumentPosition {
        page_index: new_page,
        ..pos.clone()
    }
}
