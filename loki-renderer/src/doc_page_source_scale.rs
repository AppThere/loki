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

use appthere_canvas::residency::{MAX_ZOOM, MIN_ZOOM};

use crate::doc_page_source::DocPageSource;

impl DocPageSource {
    /// Sets the paginated render zoom factor, clamped to
    /// [`MIN_ZOOM`]..=[`MAX_ZOOM`]. The next paint picks it up; the tile resize
    /// that accompanies a zoom change forces the repaint (texture-size
    /// mismatch), so no generation bump is needed.
    ///
    /// The bounds live in `appthere_canvas::residency` rather than here because
    /// they are what bounds peak texture demand — see [`MIN_ZOOM`]'s docs.
    pub fn set_zoom(&self, zoom: f32) {
        *self.zoom.lock().unwrap_or_else(|e| e.into_inner()) =
            zoom.clamp(MIN_ZOOM as f32, MAX_ZOOM as f32);
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use appthere_canvas::residency::{MAX_ZOOM, MIN_ZOOM};
    use loki_doc_model::document::Document;

    use crate::doc_page_source::DocPageSource;

    /// The clamp must be the shared residency bound, not a literal that happens
    /// to agree with it today.
    ///
    /// It *was* a literal `0.25, 4.0` here, in a crate the residency model cannot
    /// see. That mattered more than a duplicated number usually does: texture
    /// demand goes as the square of zoom, so this pair decides the peak byte
    /// figure any device can be asked for, and therefore whether the survival
    /// regime is reachable at all. Raising it here would have changed the answer
    /// to a question asked in `appthere-canvas`, with nothing connecting the two.
    ///
    /// The other half of the coupling —
    /// `plan_reachability_tests::the_sweep_covers_the_whole_clamped_range` —
    /// checks that the reachability sweep actually reaches this bound.
    #[test]
    fn the_zoom_clamp_is_the_shared_residency_bound() {
        let source = DocPageSource::new(Arc::new(Document::default()));
        source.set_zoom(1000.0);
        assert_eq!(f64::from(source.zoom()), MAX_ZOOM);
        source.set_zoom(0.0);
        assert_eq!(f64::from(source.zoom()), MIN_ZOOM);
        source.set_zoom(1.5);
        assert_eq!(source.zoom(), 1.5, "an in-range zoom must pass through");
    }
}
