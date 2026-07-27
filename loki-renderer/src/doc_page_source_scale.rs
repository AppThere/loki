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

use appthere_canvas::residency::{ZOOM_RANGE_MAX, ZOOM_RANGE_MIN};

use crate::doc_page_source::DocPageSource;

impl DocPageSource {
    /// Records the zoom the **user asked for**, clamped to
    /// [`ZOOM_RANGE_MIN`]..=[`ZOOM_RANGE_MAX`].
    ///
    /// The next paint picks it up; the tile resize that accompanies a zoom change
    /// forces the repaint (texture-size mismatch), so no generation bump is
    /// needed.
    ///
    /// # Requested and effective are separate, deliberately
    ///
    /// This stores intent. [`Self::zoom`] returns what is actually rendered,
    /// which is this value capped by [`Self::set_capability_limit_permille`].
    ///
    /// Storing only the capped value would make **every clamp permanently
    /// destroy intent**, and the clamp is not permanent — what a device can serve
    /// varies with available memory, page area and display scale, all of which
    /// move on their own. Concretely: a session at 300% that gets capped to 150%
    /// when another process takes memory would still be at 150% after the memory
    /// came back; and once page size varies per section (Spec 08 T6.1), scrolling
    /// Letter → A2 → Letter would strand the reader at the A2 limit.
    ///
    /// Keeping the request and re-applying it when the constraint lifts turns a
    /// forced reduction from *"the app changed my setting"* into *"the app
    /// temporarily reduced quality"*, which is a different event to the person
    /// experiencing it.
    ///
    /// The **range** clamp here is permanent and that is correct — it is what the
    /// control offers, not what the device can serve today.
    pub fn set_zoom(&self, zoom: f32) {
        *self.zoom.lock().unwrap_or_else(|e| e.into_inner()) =
            zoom.clamp(ZOOM_RANGE_MIN as f32, ZOOM_RANGE_MAX as f32);
    }

    /// The zoom the user asked for, ignoring any capability cap.
    ///
    /// A zoom control should display *this*, so the reader sees their own setting
    /// rather than a number the device chose for them.
    pub fn requested_zoom(&self) -> f32 {
        *self.zoom.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Caps the zoom actually rendered, in thousandths. `None` removes the cap.
    ///
    /// Supply `appthere_canvas::residency::max_servable_zoom_permille` — the
    /// bound above which a plan requests more than the OOM-calibrated survival
    /// ceiling. **Not** `max_full_scale_zoom_permille`: capping at the onset of
    /// softness would reduce a zoom the reader chose and can legibly use, which
    /// is the disruption Spec 08 T5.4 requirement 3 forbids. The gap between the
    /// two bounds is where a reader is allowed to stay.
    ///
    /// # Why this needs no separate forced-reduction path
    ///
    /// Spec 08 r34 described three regimes and gave the third — over the OOM
    /// bound — no policy beyond "the rule cannot apply". With intent stored
    /// separately there is nothing left to specify: a session pushed past the
    /// bound by falling memory has its *effective* zoom follow the cap down
    /// automatically, and back up when the cap lifts. The forced reduction and
    /// the restoration are the same mechanism, so neither can be forgotten.
    ///
    /// What is still T5.4's, because it is not storage: telling the reader it
    /// happened. A silent quality drop is the thing this makes explicable, not
    /// the thing it excuses.
    pub fn set_capability_limit_permille(&self, limit: Option<u16>) {
        *self
            .zoom_capability_permille
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = limit;
    }

    /// The capability cap currently in force, if any.
    pub fn capability_limit_permille(&self) -> Option<u16> {
        *self
            .zoom_capability_permille
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// The zoom actually rendered: [`Self::requested_zoom`] capped by any
    /// capability limit.
    ///
    /// This is what the paint path reads, so a cap takes effect without any
    /// caller remembering to apply it.
    pub fn zoom(&self) -> f32 {
        let requested = self.requested_zoom();
        match self.capability_limit_permille() {
            Some(limit) => requested.min(limit as f32 / 1000.0),
            None => requested,
        }
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

    use appthere_canvas::residency::{ZOOM_RANGE_MAX, ZOOM_RANGE_MIN};
    use loki_doc_model::document::Document;

    use crate::doc_page_source::DocPageSource;

    /// The clamp must be the shared residency bound, not a literal that happens
    /// to agree with it today.
    ///
    /// It *was* a literal `0.25, 4.0` here, in a crate the residency model cannot
    /// see. That mattered more than a duplicated number usually does: texture
    /// demand goes as the square of zoom, so this pair decides the peak byte
    /// figure any device can be asked for, and therefore whether the survival
    /// regime is reachable at all.
    ///
    /// The other half of the coupling —
    /// `plan_reachability_tests::the_sweep_covers_the_whole_clamped_range` —
    /// checks that the reachability sweep actually reaches this bound.
    #[test]
    fn the_zoom_clamp_is_the_shared_residency_bound() {
        let source = DocPageSource::new(Arc::new(Document::default()));
        source.set_zoom(1000.0);
        assert_eq!(f64::from(source.zoom()), ZOOM_RANGE_MAX);
        source.set_zoom(0.0);
        assert_eq!(f64::from(source.zoom()), ZOOM_RANGE_MIN);
        source.set_zoom(1.5);
        assert_eq!(source.zoom(), 1.5, "an in-range zoom must pass through");
    }

    /// A capability cap must reduce what is *rendered* without touching what was
    /// *asked for* — the property that makes a forced reduction recoverable.
    #[test]
    fn a_capability_cap_lowers_effective_zoom_and_leaves_the_request_intact() {
        let source = DocPageSource::new(Arc::new(Document::default()));
        source.set_zoom(3.0);
        source.set_capability_limit_permille(Some(1500));
        assert_eq!(source.zoom(), 1.5, "effective zoom must follow the cap");
        assert_eq!(
            source.requested_zoom(),
            3.0,
            "the cap must not overwrite the user's setting",
        );
    }

    /// The half that a store-the-clamped-value design gets wrong: when the
    /// constraint lifts, the reader goes back to where they were.
    ///
    /// This is the whole reason the two are separate. Without it, memory
    /// pressure at 300% strands a session at 150% permanently, and — once page
    /// size varies per section (T6.1) — scrolling Letter → A2 → Letter strands
    /// it at the A2 limit.
    #[test]
    fn lifting_the_cap_restores_the_requested_zoom() {
        let source = DocPageSource::new(Arc::new(Document::default()));
        source.set_zoom(3.0);
        source.set_capability_limit_permille(Some(1500));
        assert_eq!(source.zoom(), 1.5);
        source.set_capability_limit_permille(None);
        assert_eq!(
            source.zoom(),
            3.0,
            "removing the cap must restore the requested zoom, not leave the \
             reduced one in place",
        );
    }

    /// A cap above the request changes nothing — the common case on ordinary
    /// paper, where the device can serve the whole range.
    #[test]
    fn a_cap_above_the_request_is_inert() {
        let source = DocPageSource::new(Arc::new(Document::default()));
        source.set_zoom(1.25);
        source.set_capability_limit_permille(Some(4000));
        assert_eq!(source.zoom(), 1.25);
    }

    /// Requests still clamp to the control's range permanently. That clamp is a
    /// different kind from the capability cap — it is what the control offers,
    /// not what today's device can serve — so it *should* be destructive.
    #[test]
    fn the_range_clamp_stays_destructive_because_it_is_not_a_capability() {
        let source = DocPageSource::new(Arc::new(Document::default()));
        source.set_zoom(99.0);
        assert_eq!(f64::from(source.requested_zoom()), ZOOM_RANGE_MAX);
    }
}
