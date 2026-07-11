// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reflow-mode hit-test helpers for [`DocPageSource`]. Split from
//! `doc_page_source.rs` to keep it under the file-size ceiling.

use crate::doc_page_source::DocPageSource;
use crate::render_layout::{REFLOW_PADDING_PT, REFLOW_TILE_HEIGHT_PT};

impl DocPageSource {
    /// Hit-test a tile-local click in the reflow layout, returning
    /// `(block_index, byte_offset)`.
    ///
    /// `tile_index` is the band tile clicked; `tile_x_pt` / `tile_y_pt` are the
    /// tile-local position in layout points. Returns `None` in paginated mode or
    /// when there is no editing data at the point.
    pub fn reflow_hit_test(
        &self,
        tile_index: usize,
        tile_x_pt: f32,
        tile_y_pt: f32,
    ) -> Option<(usize, usize)> {
        let guard = self.layout_for_generation(self.current_generation());
        let (_, layout) = guard.as_ref()?;
        // Tile-local → canvas: undo the band's x inset and y offset.
        let canvas_x = tile_x_pt - REFLOW_PADDING_PT;
        let canvas_y = tile_y_pt + tile_index as f32 * REFLOW_TILE_HEIGHT_PT;
        layout.reflow_hit_test(canvas_x, canvas_y)
    }

    /// Hyperlink URL under a tile-local point in the reflow layout, or `None`
    /// in paginated mode / over plain text. Coordinates as in
    /// [`Self::reflow_hit_test`].
    pub fn reflow_link_at(
        &self,
        tile_index: usize,
        tile_x_pt: f32,
        tile_y_pt: f32,
    ) -> Option<String> {
        let guard = self.layout_for_generation(self.current_generation());
        let (_, layout) = guard.as_ref()?;
        let canvas_x = tile_x_pt - REFLOW_PADDING_PT;
        let canvas_y = tile_y_pt + tile_index as f32 * REFLOW_TILE_HEIGHT_PT;
        layout.reflow_link_at(canvas_x, canvas_y)
    }

    /// The reflow band (tile) index containing the caret for `(block_index,
    /// byte_offset)`, or `None` in paginated mode / when not found.
    ///
    /// The view uses this as the caret's `page_index` so the correct tile is
    /// invalidated (and repainted) as the caret moves between bands.
    pub fn reflow_cursor_band(&self, block_index: usize, byte_offset: usize) -> Option<usize> {
        let guard = self.layout_for_generation(self.current_generation());
        let (_, layout) = guard.as_ref()?;
        let cr = layout.reflow_cursor_canvas(block_index, byte_offset)?;
        Some((cr.y / REFLOW_TILE_HEIGHT_PT).floor().max(0.0) as usize)
    }
}
