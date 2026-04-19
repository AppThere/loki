// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Spacing scale design tokens (CSS pixels as `f32`).
//!
//! The scale uses a 4 px base unit.  Token names follow the pattern
//! `SPACE_<multiplier>` where the multiplier corresponds to the number of
//! 4 px steps (e.g. `SPACE_2` = 2 × 4 px = 8 px).

// Token constants may not all be referenced in every build state.
#![allow(dead_code)]

/// 4 px — tightest spacing unit; micro gaps within components.
pub const SPACE_1: f32 = 4.0;

/// 8 px — small internal padding.
pub const SPACE_2: f32 = 8.0;

/// 12 px — medium internal padding.
pub const SPACE_3: f32 = 12.0;

/// 16 px — standard section padding and inter-component gaps.
pub const SPACE_4: f32 = 16.0;

/// 24 px — larger section separation.
pub const SPACE_6: f32 = 24.0;

/// 32 px — wide outer gutters on desktop viewports.
pub const SPACE_8: f32 = 32.0;
