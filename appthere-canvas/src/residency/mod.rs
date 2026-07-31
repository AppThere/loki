// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Resident page-texture accounting (Spec 08 Phase 2, T2.5).
//!
//! # What this is for
//!
//! Spec 08 r1 asserted that resident page textures scale with document length.
//! Spike S0.2 found they do not: tiles are already windowed to the viewport
//! neighbourhood, so the resident set is bounded by *mounting*, and the axis
//! that is actually unbounded is **zoom × device scale factor** — a page
//! texture is `(css_px · zoom · dsf)² · 4` bytes, and both factors are under
//! the user's thumb. Phase 2 bounds that axis; document-length scaling is
//! Spec 09's I-18 and explicitly out of scope here.
//!
//! # Two halves, deliberately separated
//!
//! - [`geometry`] is the **model**: which pages mount, how large each one's
//!   texture is, and therefore how many bytes are resident. It is pure
//!   arithmetic over page sizes and viewport geometry, so it runs headless and
//!   is the thing Phase 2's bench sweeps.
//! - [`counter`] is the **instrument**: a process-wide byte counter fed by the
//!   real allocation and release call sites in `loki-renderer`. It is what
//!   makes the budget assertable in a test rather than inferred, and what an
//!   on-device run reads to confirm the model.
//!
//! The model is not a second implementation of the mounting rule: production's
//! virtualization calls [`geometry::visible_window`] directly. A model that
//! restated the rule would drift from it, and a drifted model measures itself.
//!
//! # Why the geometry lives here and not in `loki-renderer`
//!
//! `loki-renderer` owns the `wgpu` call that allocates, but this crate owns the
//! texture types every allocation produces ([`crate::GpuTexture`],
//! [`crate::PageSource`]), is free of `loki-*` dependencies, and is already the
//! shared canvas layer for AppThere applications. Keeping the arithmetic here
//! also keeps it reachable from a headless bench without pulling in Dioxus,
//! Blitz and wgpu. See ADR-0016 for the full decision (Spec 08 T2.4a).
//!
//! # What a byte figure from here does and does not include
//!
//! It is **requested** bytes: width × height × 4 for the `Rgba8Unorm` page
//! textures, summed over the mounted set. It excludes driver-side overhead —
//! row-pitch alignment, heap granularity, any implicit staging — which is not
//! observable without a device and is what Spec 08 R16's on-device run
//! confirms. It also excludes Vello's own scratch buffers, which are shared
//! across tiles rather than per-tile.
//!
//! # The accessibility text scale must never reach page pixels (Spec 08 I-24)
//!
//! Texture demand is `page_px × zoom × display_scale` — three inputs, and a
//! platform text-size setting is not among them. I-24 will make *chrome*
//! respond to that setting; **document content must not**, and this is where
//! the cost of getting it wrong would land. A 2.0× term reaching page pixels
//! multiplies area by ~4 on the one axis Phase 2 spent a whole phase bounding:
//! §3.6b measured the survival ceiling as reachable on **every** memory size
//! from 2 to 64 GiB inside the app's own zoom clamp, so the ceiling would begin
//! firing at ordinary zoom. The symptom — soft text at rest — reads as a memory
//! regression, so it would be chased here rather than in the accessibility
//! change that caused it.
//!
//! Two mechanisms could carry the term across: a crate dependency on
//! `appthere-ui`, or a font-relative CSS unit (`rem`/`em`/`ch`/`lh`) sizing the
//! canvas subtree. Both are refused mechanically by
//! `scripts/check-document-scale-isolation.py` rather than by convention — the
//! boundary was correct already, but it rested on nobody wiring the two
//! together.

mod budget;
mod budget_derive;
mod counter;
mod geometry;
mod plan;
mod plan_capability;

pub use budget::{
    BUDGET_BASELINE_BYTES, BUDGET_CEILING_BYTES, BUDGET_FLOOR_BYTES, BudgetInputs, BudgetSource,
    SURVIVAL_AVAILABLE_RAM_DIVISOR, SURVIVAL_CAP_BYTES, SURVIVAL_TOTAL_RAM_DIVISOR, TextureBudget,
};
pub use counter::{TextureResidency, TextureResidencySnapshot};
pub use geometry::{
    BYTES_PER_TEXEL, PT_TO_CSS_PX, PageBox, ViewportSpec, ZOOM_RANGE_MAX, ZOOM_RANGE_MIN,
    ZOOM_RANGE_MIN_PERMILLE, resident_pages, resident_texture_bytes, texture_bytes, visible_window,
    zoom_from_permille,
};
pub use plan::{
    MIN_RASTER_SCALE, RASTER_SCALE_LADDER, ResidencyPlan, TilePlan, plan_residency,
    strictly_visible,
};
pub use plan_capability::{
    ZOOM_PROBE_STEP_PERMILLE, is_servable_at_all, max_full_scale_zoom_permille,
    max_servable_zoom_permille,
};
