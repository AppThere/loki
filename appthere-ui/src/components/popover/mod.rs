// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The shared overlay primitive (Spec 08 T4.1).

pub mod geometry;
pub mod interaction;

pub use geometry::{place, Align, Placement, PlacementRequest, Rect, Side};
