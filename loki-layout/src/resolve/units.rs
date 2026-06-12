// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit conversion helpers.

use loki_primitives::units::Points;

/// Convert a [`Points`] value to `f32`.
pub fn pts_to_f32(pts: Points) -> f32 {
    pts.value() as f32
}

/// Convert English Metric Units (EMU) to points. 1 EMU = 1/12700 pt.
///
/// OOXML stores image dimensions in EMU. This converts to the `f32` points
/// used by `loki-layout` geometry types.
pub fn emu_to_pt(emu: u64) -> f32 {
    emu as f32 / 12700.0
}
