// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Multi-column flow helpers for the paginated engine.
//!
//! A section with `columns > 1` flows content down each column (a full-height
//! band of width [`FlowState::content_width`]) before advancing to the next
//! column, and only starts a new page once the last column is full. Items are
//! placed with x relative to the *current column's* left edge and shifted to the
//! column's absolute offset when the column is finished — see
//! [`position_current_column`].

use super::{FlowState, finish_page};
use crate::color::LayoutColor;
use crate::geometry::LayoutRect;
use crate::items::{PositionedItem, PositionedRect};
use crate::resolve::pts_to_f32;

/// Resolves a section's column model into `(count, gap, separator, widths)`.
/// Columns only apply in paginated mode; unequal per-column widths are honoured
/// when one is supplied for every column, otherwise the content area is split
/// equally.
pub(super) fn column_layout_for(
    columns: Option<&loki_doc_model::layout::page::SectionColumns>,
    full_content_width: f32,
    paginated: bool,
) -> (u8, f32, bool, Vec<f32>) {
    match columns {
        Some(c) if c.count >= 2 && paginated => {
            let n = f32::from(c.count);
            let gap = pts_to_f32(c.gap);
            let widths = if c.widths.len() == usize::from(c.count) {
                c.widths.iter().map(|w| pts_to_f32(*w).max(0.0)).collect()
            } else {
                let col_w = ((full_content_width - (n - 1.0) * gap) / n).max(0.0);
                vec![col_w; usize::from(c.count)]
            };
            (c.count, gap, c.separator, widths)
        }
        _ => (1, 0.0, false, vec![full_content_width]),
    }
}

/// Horizontal offset of column `state.col_index`'s left edge from the
/// content-area left: the running sum of all preceding column widths plus one
/// gap per boundary crossed. Handles unequal columns; returns 0 for
/// single-column flows.
fn column_x_offset(state: &FlowState) -> f32 {
    if state.columns <= 1 {
        return 0.0;
    }
    let i = usize::from(state.col_index);
    let widths: f32 = state.column_widths.iter().take(i).sum();
    widths + state.col_index as f32 * state.column_gap
}

/// Shifts the items and editing data placed in the current column (everything
/// from `column_item_start` / `column_para_start` onward) to that column's
/// horizontal offset. A no-op for column 0 and for single-column flows.
pub(super) fn position_current_column(state: &mut FlowState) {
    let off = column_x_offset(state);
    if off == 0.0 {
        return;
    }
    for item in &mut state.current_items[state.column_item_start..] {
        item.translate(off, 0.0);
    }
    for para in &mut state.current_paragraphs[state.column_para_start..] {
        para.origin.0 += off;
    }
}

/// Called when the current column has run out of vertical space. Advances to the
/// next column on the same page when one remains; otherwise finishes the page.
///
/// This is what mid-flow space-exhaustion uses (instead of [`finish_page`]) so
/// that text fills every column before a page break. Explicit page breaks
/// (`page_break_before`/`after`) still call [`finish_page`] directly to skip any
/// remaining columns. For single-column flows this is exactly [`finish_page`].
pub(super) fn break_column(state: &mut FlowState) {
    if state.columns > 1 && u16::from(state.col_index) + 1 < u16::from(state.columns) {
        position_current_column(state);
        state.col_index += 1;
        // Adopt the new column's width (columns may be unequal).
        state.content_width = state
            .column_widths
            .get(usize::from(state.col_index))
            .copied()
            .unwrap_or(state.content_width);
        state.column_item_start = state.current_items.len();
        state.column_para_start = state.current_paragraphs.len();
        // The next column starts at the band top (page content top, or mid-page
        // for a continuous section that opened its band below earlier content).
        state.cursor_y = state.column_top_y;
    } else {
        finish_page(state);
    }
}

/// Emits a vertical separator line in each inter-column gap that has content on
/// this page (gaps `0..col_index`). Lines run the full content height; a no-op
/// unless the section requested separators and at least two columns were used.
pub(super) fn emit_column_separators(state: &mut FlowState) {
    if state.columns <= 1 || !state.column_separator || state.col_index == 0 {
        return;
    }
    const SEP_WIDTH: f32 = 0.75;
    // Separators span the column band: from its top (mid-page for a continuous
    // section) to the page content bottom.
    let top = state.column_top_y;
    let height = (state.page_content_height - top).max(0.0);
    for gap_idx in 0..state.col_index {
        // The separator sits in the middle of the gap after column `gap_idx`:
        // the sum of widths through that column plus `gap_idx` gaps, plus half
        // the current gap. Handles unequal columns.
        let widths: f32 = state
            .column_widths
            .iter()
            .take(usize::from(gap_idx) + 1)
            .sum();
        let center = widths + f32::from(gap_idx) * state.column_gap + state.column_gap / 2.0;
        state
            .current_items
            .push(PositionedItem::FilledRect(PositionedRect {
                rect: LayoutRect::new(center - SEP_WIDTH / 2.0, top, SEP_WIDTH, height),
                color: LayoutColor::BLACK,
            }));
    }
}
