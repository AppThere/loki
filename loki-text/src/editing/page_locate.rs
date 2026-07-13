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

/// Geometry tolerance for the content-band fit checks (points).
const BAND_EPSILON: f32 = 0.5;

/// Returns `pos` with its `page_index` re-derived from `layout`.
///
/// When the position's paragraph is found on exactly one page, that page
/// wins. When it spans several pages (a split paragraph), the page whose
/// content band **fully contains** the byte offset's line wins — the split
/// engine moves a non-fitting line entirely to the next page, so only the
/// page that actually renders the line satisfies this. (A centre-only check
/// mis-attributed the first line after a page break to the *previous* page
/// whenever that page ended with more than half a line of slack, painting
/// the caret near the bottom of the wrong page.) If no page fully contains
/// the line (degenerate geometry, e.g. a line taller than the band), the
/// line-centre rule decides; failing that, the first page holding the
/// paragraph is used. When the paragraph is not in the layout at all (e.g.
/// the layout is momentarily stale), `pos` is returned unchanged.
#[must_use]
pub fn recompute_page_index(layout: &PaginatedLayout, pos: &DocumentPosition) -> DocumentPosition {
    let mut first_holder: Option<usize> = None;
    let mut center_match: Option<usize> = None;
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
        if let Some(rect) = para.layout.cursor_rect(pos.byte_offset) {
            // Line extent in page-content coordinates (the per-page origin is
            // already content-relative).
            let y_top = rect.y + para.origin.1;
            let y_bottom = y_top + rect.height;
            let content_h = page.page_size.height - page.margins.top - page.margins.bottom;
            if y_top >= -BAND_EPSILON && y_bottom <= content_h + BAND_EPSILON {
                visible = Some(pi);
            } else if center_match.is_none() {
                let y_center = y_top + rect.height / 2.0;
                if y_center >= 0.0 && y_center < content_h {
                    center_match = Some(pi);
                }
            }
        }
        if visible.is_some() {
            break;
        }
    }

    let new_page = match (visible, center_match, first_holder) {
        (Some(pi), _, _) => pi,
        (None, Some(pi), _) => pi,
        (None, None, Some(pi)) => pi,
        (None, None, None) => pos.page_index,
    };
    if new_page == pos.page_index {
        return pos.clone();
    }
    DocumentPosition {
        page_index: new_page,
        ..pos.clone()
    }
}
