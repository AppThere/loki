// SPDX-License-Identifier: Apache-2.0

//! Typography scale design tokens (font sizes as `f32` CSS pixels, weights as `&str`).

// Token constants may not all be referenced in every build stage.
#![allow(dead_code)]

// ── Font family ───────────────────────────────────────────────────────────────

/// UI font stack. Atkinson Hyperlegible Next is the primary face; the fallbacks
/// ensure legibility if it is not installed.
pub const FONT_FAMILY_UI: &str =
    "Atkinson Hyperlegible Next, Atkinson Hyperlegible, system-ui, sans-serif";

// ── Font sizes (CSS pixels as f32) ────────────────────────────────────────────

/// Extra-small — status bar labels, timestamp metadata.
pub const FONT_SIZE_XS: f32 = 11.0;

/// Label / caption — metadata, timestamps, and secondary info.
pub const FONT_SIZE_LABEL: f32 = 12.0;

/// Body text — paragraph copy and list content.
pub const FONT_SIZE_BODY: f32 = 14.0;

/// Medium — slightly larger body text, e.g. card titles.
pub const FONT_SIZE_MD: f32 = 15.0;

/// Heading — section titles within a screen.
pub const FONT_SIZE_HEADING: f32 = 20.0;

// ── Font weights (CSS weight strings) ─────────────────────────────────────────

/// Regular weight — body copy and secondary labels.
pub const FONT_WEIGHT_REGULAR: &str = "400";

/// Medium weight — slightly emphasised text.
pub const FONT_WEIGHT_MEDIUM: &str = "500";

/// Semibold weight — section headings, card titles, tab labels.
pub const FONT_WEIGHT_SEMIBOLD: &str = "600";

/// Bold weight — primary headings, app name.
pub const FONT_WEIGHT_BOLD: &str = "700";
