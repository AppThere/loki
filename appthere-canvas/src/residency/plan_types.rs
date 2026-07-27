// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! What `plan_residency` returns: one tile's mounting decision, and the whole
//! plan's.
//!
//! Split from `plan.rs` at the 300-line ceiling. The seam is the usual one —
//! the shapes the algorithm produces, apart from the algorithm — and it earns
//! its keep here because [`ResidencyPlan`]'s three flags carry more explanation
//! than the steps that set them: each names a different *kind* of outcome, and a
//! reader of a diagnostic has to be able to tell them apart.

/// One tile in a residency plan.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TilePlan {
    /// Page index in document order.
    pub page_index: usize,
    /// Rasterisation scale to allocate this tile's texture at, in `(0, 1]`.
    pub raster_scale: f32,
    /// Requested texture bytes at that scale.
    pub bytes: u64,
    /// Whether the page overlaps the visible rect, as opposed to the grown
    /// window. Visible tiles are never dropped.
    pub visible: bool,
}

/// The mounting decision for one viewport state.
#[derive(Clone, PartialEq, Debug)]
pub struct ResidencyPlan {
    /// Tiles to mount, in document order.
    pub tiles: Vec<TilePlan>,
    /// Requested texture bytes across [`Self::tiles`].
    pub total_bytes: u64,
    /// `true` when the plan exceeds the byte **target** after spending
    /// everything the target is allowed to spend.
    ///
    /// Reported rather than resolved, and since r15 this is an ordinary outcome
    /// rather than a corner: the visible set at full scale can exceed a derived
    /// target during normal reading on a HiDPI machine, and the correct response
    /// is to say so, not to soften the page. See the module docs for why.
    pub over_target: bool,
    /// `true` when the visible set's scale was reduced to stay under the
    /// **survival ceiling** — the only circumstance in which this planner
    /// degrades what the user is looking at.
    ///
    /// Distinct from [`Self::over_target`] because they carry opposite meanings
    /// for a reader of a diagnostic: over-target is the design working, while
    /// this is the device having run out of room.
    pub survival_reduced: bool,
    /// `true` when the visible set exceeds the **survival ceiling** even at
    /// [`super::MIN_RASTER_SCALE`] — the plan requests more than the threshold
    /// calibrated against the OOM killer, and knows it.
    ///
    /// # This is not a worse [`Self::survival_reduced`]
    ///
    /// Every other outcome in this planner is a concession it chose. This one is
    /// a concession it could not make: the scale search hit its floor and the
    /// plan was mounted anyway. Until r30 that happened *silently*, which is the
    /// worst of the available behaviours — worse than a soft page, worse than a
    /// visible failure — because the one threshold that exists to prevent the
    /// process being killed was crossed with nothing said.
    ///
    /// Reachable today, not pending a page-size catalogue: only the `PageBox`
    /// *constructors* are limited to A4 and Letter, while imported geometry is
    /// arbitrary, so an A1 or A0 document reaches this on a constrained HiDPI
    /// device — and A0 reaches it on 16 GiB at 4x. See `plan_ceiling_tests`.
    ///
    /// What the application should *do* about it is not decided here and is not
    /// this type's business; what is decided is that it will not be hidden.
    pub ceiling_exceeded: bool,
}

impl ResidencyPlan {
    /// The scale for `page_index`, or `None` when the page is not mounted.
    #[must_use]
    pub fn raster_scale(&self, page_index: usize) -> Option<f32> {
        self.tiles
            .iter()
            .find(|t| t.page_index == page_index)
            .map(|t| t.raster_scale)
    }
}
