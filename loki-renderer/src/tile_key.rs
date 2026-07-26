// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! When a page tile's texture may be reused, and when it must be re-rendered
//! (Spec 08 T2.3, risk R10).
//!
//! # Why this is a type and not four `&&`s in the render callback
//!
//! It was four `&&`s, inside a method that needs a live wgpu device — so the
//! one thing R10 asks for, an explicit zoom-invalidation test, could not be
//! written. Reuse is a pure comparison; pulling it out makes the invalidation
//! rule the thing that is tested rather than the thing that is inspected.
//!
//! # What each field covers, against T2.3's list
//!
//! | Trigger | Field |
//! | --- | --- |
//! | Edit | `generation` — every mutation advances it |
//! | Page-style change | `generation`, and `size_px` when the paper size moves |
//! | Zoom | `size_px` — the tile box is `pt × 96/72 × zoom` |
//! | Device scale factor | `size_px` — Blitz hands the source physical pixels |
//! | Caret / selection | `cursor` |
//! | Budget pressure | `raster_permille` (T2.2) |
//!
//! Zoom and device scale factor both land in `size_px` rather than being
//! tracked separately, and that is the interesting case: they are not
//! independently recoverable from it — 200% at 1× and 100% at 2× produce the
//! same pixels, and correctly reuse the same texture, because the texture is
//! the same. The key is over what a texture *is*, not over what produced it.

use crate::document_view::RendererSelection;

/// Everything a page tile's texture depends on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct TileKey {
    /// Document generation the texture was rendered at.
    pub(crate) generation: u64,
    /// Physical texture dimensions.
    pub(crate) size_px: (u32, u32),
    /// Caret and selection at render time.
    pub(crate) cursor: Option<RendererSelection>,
    /// Rasterisation scale in thousandths.
    ///
    /// Quantised because it is a comparison key: an `f32` would make
    /// `PartialEq` depend on exact bit patterns, and a scale arriving from a
    /// square root would re-render a tile every frame on the last mantissa bit.
    pub(crate) raster_permille: u16,
}

impl TileKey {
    /// Builds a key, quantising `raster_scale` to thousandths.
    pub(crate) fn new(
        generation: u64,
        size_px: (u32, u32),
        cursor: Option<RendererSelection>,
        raster_scale: f32,
    ) -> Self {
        Self {
            generation,
            size_px,
            cursor,
            raster_permille: quantise(raster_scale),
        }
    }
}

/// Rasterisation scale as thousandths, clamped to `[1, 1000]`.
///
/// The floor is 1 rather than 0: a zero-scale tile would be a zero-sized
/// texture, which is a different bug from a very small one.
pub(crate) fn quantise(raster_scale: f32) -> u16 {
    if !raster_scale.is_finite() {
        return 1000;
    }
    ((raster_scale * 1000.0).round() as i32).clamp(1, 1000) as u16
}

#[cfg(test)]
#[path = "tile_key_tests.rs"]
mod tests;
