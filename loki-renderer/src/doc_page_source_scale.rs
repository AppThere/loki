// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The two render-scale factors [`DocPageSource`] carries for the paint source:
//! the user's zoom and the texture budget's rasterisation scale. Split from
//! `doc_page_source.rs` to keep it under the file-size ceiling.
//!
//! They belong together because they are the same kind of thing — multipliers
//! the paint path reads *through the shared source* rather than from props,
//! since `LokiPageSource` is created once by `use_wgpu` and never sees a later
//! prop update. They multiply into different places, though: zoom scales the
//! tile's on-screen box *and* its texture, while the rasterisation scale
//! shrinks only the texture and leaves the box alone. That asymmetry is the
//! whole of Spec 08 T2.2.

use crate::doc_page_source::DocPageSource;

impl DocPageSource {
    /// Sets the paginated render zoom factor (clamped to a sane range). The
    /// next paint picks it up; the tile resize that accompanies a zoom change
    /// forces the repaint (texture-size mismatch), so no generation bump is
    /// needed.
    pub fn set_zoom(&self, zoom: f32) {
        *self.zoom.lock().unwrap_or_else(|e| e.into_inner()) = zoom.clamp(0.25, 4.0);
    }

    /// The current paginated render zoom factor.
    pub fn zoom(&self) -> f32 {
        *self.zoom.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Records the rasterisation scale (in thousandths of full) the texture
    /// budget allows page `page_index` (Spec 08 T2.2).
    ///
    /// Thousandths rather than `f32` because the value is compared for equality
    /// in the tile's reuse key; see `tile_key::quantise`.
    pub fn set_raster_permille(&self, page_index: usize, permille: u16) {
        let mut guard = self
            .raster_permille
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        guard.insert(page_index, permille.clamp(1, 1000));
    }

    /// The rasterisation scale for `page_index`, in thousandths. Full scale when
    /// the planner has not said otherwise, so a path with no budget applied
    /// renders exactly as it did before Phase 2.
    pub fn raster_permille(&self, page_index: usize) -> u16 {
        self.raster_permille
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&page_index)
            .copied()
            .unwrap_or(1000)
    }
}
