// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Design tokens for `loki-text`.
//!
//! All visual constants live here. Component files **must not** embed magic
//! numbers; they must reference these constants instead. This ensures a single
//! source of truth for the visual language.
//!
//! # Philosophy
//!
//! Loki uses a **Flat Design** aesthetic: solid fills, clear typographic
//! hierarchy, and a single primary accent color. No gradients, no box shadows.

// Design tokens may not all be referenced in every build state.  New
// components will consume them as the UI grows.
#![allow(dead_code)]

// ── Color palette ─────────────────────────────────────────────────────────────

/// Pure white page surface.
pub const COLOR_PAGE_WHITE: &str = "#FFFFFF";

/// Light gray application background (behind document pages and cards).
pub const COLOR_SURFACE: &str = "#F5F5F5";

/// Subtle border color for dividers and card outlines.
pub const COLOR_BORDER: &str = "#E0E0E0";

/// Primary accent — used for buttons, active states, and key UI highlights.
pub const COLOR_ACCENT: &str = "#1976D2";

/// Darker accent variant shown on hover over accent-colored interactive elements.
pub const COLOR_ACCENT_HOVER: &str = "#1565C0";

/// Primary text color for headings and body copy.
pub const COLOR_TEXT_PRIMARY: &str = "#212121";

/// Secondary text color for labels, captions, and metadata.
pub const COLOR_TEXT_SECONDARY: &str = "#757575";

/// Error / warning fill color (used in inline error banners).
pub const COLOR_ERROR_BG: &str = "#FFEBEE";

/// Error border and text color.
pub const COLOR_ERROR_TEXT: &str = "#C62828";

/// Error border color.
pub const COLOR_ERROR_BORDER: &str = "#EF9A9A";

// ── Spacing scale (px) ────────────────────────────────────────────────────────

/// 4 px — tightest spacing unit; used for micro gaps within components.
pub const SPACING_4: f32 = 4.0;

/// 8 px — small internal padding.
pub const SPACING_8: f32 = 8.0;

/// 12 px — medium internal padding.
pub const SPACING_12: f32 = 12.0;

/// 16 px — standard section padding and inter-component gaps.
pub const SPACING_16: f32 = 16.0;

/// 24 px — larger section separation.
pub const SPACING_24: f32 = 24.0;

/// 32 px — wide outer gutters on desktop viewports.
pub const SPACING_32: f32 = 32.0;

// ── Type scale (px) ───────────────────────────────────────────────────────────

/// Body text size — used for paragraph copy and list content.
pub const FONT_SIZE_BODY: f32 = 14.0;

/// Label / caption size — used for metadata, timestamps, and secondary info.
pub const FONT_SIZE_LABEL: f32 = 12.0;

/// Heading size — used for section titles within a screen.
pub const FONT_SIZE_HEADING: f32 = 20.0;

// ── Toolbar heights (px) ──────────────────────────────────────────────────────

/// Height of the top toolbar in the editor shell.
pub const TOOLBAR_HEIGHT_TOP: f32 = 48.0;

/// Height of the bottom status bar in the editor shell.
pub const TOOLBAR_HEIGHT_BOTTOM: f32 = 40.0;

// ── Document page dimensions (A4 at 96 dpi) ──────────────────────────────────

/// A4 page width in CSS pixels at 96 dpi equivalent (210 mm → ~794 px).
pub const PAGE_WIDTH_PX: f32 = 794.0;

/// A4 page height in CSS pixels at 96 dpi equivalent (297 mm → ~1123 px).
pub const PAGE_HEIGHT_PX: f32 = 1123.0;

// ── Responsive breakpoints ────────────────────────────────────────────────────

/// Viewport width above which the UI switches to the desktop two-column layout.
pub const DESKTOP_BREAKPOINT: f32 = 768.0;

/// Maximum width for primary action buttons on desktop (centered, fixed width).
pub const DESKTOP_BUTTON_MAX_WIDTH: f32 = 320.0;
