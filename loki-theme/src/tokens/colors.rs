// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Color palette design tokens.
//!
//! All values are CSS hex strings suitable for inline `style` attributes.
//! Loki uses a **Flat Design** aesthetic: solid fills, no gradients, no shadows.

// Token constants are consumed by multiple crates at different build stages.
#![allow(dead_code)]

// ── Surface colors ────────────────────────────────────────────────────────────

/// Pure white — document page surface and card backgrounds.
pub const COLOR_SURFACE_PAGE: &str = "#FFFFFF";

/// Light gray — application background behind document pages and cards.
pub const COLOR_SURFACE_BASE: &str = "#F5F5F5";

// ── Border ────────────────────────────────────────────────────────────────────

/// Default border color for dividers and card outlines.
pub const COLOR_BORDER_DEFAULT: &str = "#E0E0E0";

// ── Accent ────────────────────────────────────────────────────────────────────

/// Primary accent — buttons, active states, and key UI highlights.
pub const COLOR_ACCENT_PRIMARY: &str = "#3D7EFF";

/// Darker accent shown on hover over primary accent elements.
pub const COLOR_ACCENT_PRIMARY_HOVER: &str = "#3771E6";

// ── Text ──────────────────────────────────────────────────────────────────────

/// Primary text — headings and body copy.
pub const COLOR_TEXT_PRIMARY: &str = "#1A1A1A";

/// Secondary text — labels, captions, and metadata.
pub const COLOR_TEXT_SECONDARY: &str = "#6B6B6B";

// ── Status: error ─────────────────────────────────────────────────────────────

/// Error banner background fill.
pub const COLOR_STATUS_ERROR_BG: &str = "#FFEBEE";

/// Error banner text and icon color.
pub const COLOR_STATUS_ERROR_TEXT: &str = "#C62828";

/// Error banner border color.
pub const COLOR_STATUS_ERROR_BORDER: &str = "#EF9A9A";
