// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Typed 2D geometry primitives.

mod insets;
mod point;
mod rect;
mod size;
mod transform;

pub use insets::Insets;
pub use point::Point;
pub use rect::Rect;
pub use size::Size;
pub use transform::Affine2;
