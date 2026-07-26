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

mod counter;
mod geometry;

pub use counter::{TextureResidency, TextureResidencySnapshot};
pub use geometry::{
    resident_pages, resident_texture_bytes, texture_bytes, visible_window, PageBox, ViewportSpec,
    BYTES_PER_TEXEL, PT_TO_CSS_PX,
};
