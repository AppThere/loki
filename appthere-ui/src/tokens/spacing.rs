// SPDX-License-Identifier: Apache-2.0

//! Spacing scale design tokens (CSS pixels as `f32`).
//!
//! The scale uses a 4 px base unit. Token names follow the pattern
//! `SPACE_<multiplier>` where the multiplier is the number of 4 px steps
//! (e.g. `SPACE_2` = 2 × 4 px = 8 px).

// Token constants may not all be referenced in every build stage.
#![allow(dead_code)]

// ── Spacing scale ─────────────────────────────────────────────────────────────

/// 4 px — tightest spacing unit; micro gaps within components.
pub const SPACE_1: f32 = 4.0;

/// 8 px — small internal padding.
pub const SPACE_2: f32 = 8.0;

/// 12 px — medium internal padding.
pub const SPACE_3: f32 = 12.0;

/// 16 px — standard section padding and inter-component gaps.
pub const SPACE_4: f32 = 16.0;

/// 20 px — slightly wider padding.
pub const SPACE_5: f32 = 20.0;

/// 24 px — larger section separation.
pub const SPACE_6: f32 = 24.0;

/// 32 px — wide outer gutters on desktop viewports.
pub const SPACE_8: f32 = 32.0;

/// 40 px — extra-wide spacing for large layout gaps.
pub const SPACE_10: f32 = 40.0;

// ── Border radius ─────────────────────────────────────────────────────────────

/// 4 px border-radius — buttons, input fields, small cards.
pub const RADIUS_SM: f32 = 4.0;

/// 6 px border-radius — medium cards, dialogs.
pub const RADIUS_MD: f32 = 6.0;

/// 10 px border-radius — large cards, panels.
pub const RADIUS_LG: f32 = 10.0;

/// 16 px border-radius — extra-large surface overlays.
pub const RADIUS_XL: f32 = 16.0;

/// 9999 px border-radius — pill / fully-rounded elements.
pub const RADIUS_FULL: f32 = 9999.0;

// ── Touch targets ─────────────────────────────────────────────────────────────

/// Minimum touch target size per WCAG 2.5.8 (44 × 44 logical pixels).
pub const TOUCH_MIN: f32 = 44.0;

// ── Icon sizes ────────────────────────────────────────────────────────────────

/// 16 px — small icon (inline, compact toolbar).
pub const ICON_SIZE_SM: f32 = 16.0;

/// 20 px — medium icon (standard toolbar button).
pub const ICON_SIZE_MD: f32 = 20.0;

/// 24 px — large icon (title bar, prominent actions).
pub const ICON_SIZE_LG: f32 = 24.0;

/// 32 px — extra-large icon (app icon, empty-state illustration).
pub const ICON_SIZE_XL: f32 = 32.0;
