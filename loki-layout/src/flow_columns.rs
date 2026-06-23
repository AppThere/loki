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

/// Horizontal offset of column `state.col_index`'s left edge from the
/// content-area left. Column 0 is at offset 0; each later column is shifted by
/// one column width plus the gap. Returns 0 for single-column flows.
fn column_x_offset(state: &FlowState) -> f32 {
    if state.columns <= 1 {
        return 0.0;
    }
    f32::from(state.col_index) * (state.content_width + state.column_gap)
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
        state.column_item_start = state.current_items.len();
        state.column_para_start = state.current_paragraphs.len();
        state.cursor_y = 0.0;
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
    let col_w = state.content_width;
    let height = state.page_content_height;
    for gap_idx in 0..state.col_index {
        let center =
            f32::from(gap_idx) * (col_w + state.column_gap) + col_w + state.column_gap / 2.0;
        state
            .current_items
            .push(PositionedItem::FilledRect(PositionedRect {
                rect: LayoutRect::new(center - SEP_WIDTH / 2.0, 0.0, SEP_WIDTH, height),
                color: LayoutColor::BLACK,
            }));
    }
}
