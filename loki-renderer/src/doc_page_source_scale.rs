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
    ///
    /// # It discards any memo
    ///
    /// [`Self::apply_capability_limit`] stores "this limit is what those inputs
    /// produce", and after a direct set it no longer does —
    /// so the inputs are cleared with it and the next
    /// [`Self::apply_capability_limit`] recomputes rather than believing a limit
    /// nothing derived.
    pub fn set_capability_limit_permille(&self, limit: Option<u16>) {
        *self
            .zoom_capability_permille
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = (None, limit);
    }

    /// Recomputes the cap **only when its inputs have changed**.
    ///
    /// # The bound is expensive and the render path asks for it every frame
    ///
    /// `max_servable_zoom_permille` searches the real planner: 76 zoom probes ×
    /// 40 scroll offsets is up to **3040 `plan_residency` calls over an 8-page
    /// document** for one answer. Searching rather than solving is the right
    /// call — a closed form would be a second derivation of decisions
    /// `plan_residency` already makes — but `scale_resolve::resolve` runs on
    /// every render, so it was paying that search on every scroll frame to
    /// re-derive a number that had not moved.
    ///
    /// It cannot have moved without one of these changing: the bound is a pure
    /// function of the largest page, the display scale and the texture budget.
    /// The document's pages change on a layout generation, the display scale on
    /// a monitor change, and the budget on the 5-second memory probe — so the
    /// steady state during a scroll is *no change at all*.
    ///
    /// # Why the inputs live in this lock rather than beside it
    ///
    /// A memo whose key is stored separately from its value has two writers and
    /// one invariant between them, which is the shape that goes stale (L08-029).
    /// Here the pair moves together under one lock and there is no ordering for
    /// a caller to get wrong: this method is the only writer of both, and the
    /// direct setter above clears the key rather than leaving one that lies.
    ///
    /// `compute` is a closure rather than a value so the search is not run to
    /// produce an argument that is then thrown away — the whole point is that it
    /// does not run.
    pub fn apply_capability_limit(
        &self,
        inputs: crate::zoom_capability::CapabilityInputs,
        compute: impl FnOnce() -> Option<u16>,
    ) {
        let mut slot = self
            .zoom_capability_permille
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if slot.0 == Some(inputs) {
            return;
        }
        *slot = (Some(inputs), compute());
    }

    /// The capability cap currently in force, if any.
    pub fn capability_limit_permille(&self) -> Option<u16> {
        self.zoom_capability_permille
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .1
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
#[path = "doc_page_source_scale_tests.rs"]
mod tests;
