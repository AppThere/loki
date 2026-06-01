// SPDX-License-Identifier: Apache-2.0

//! Color palette design tokens.
//!
//! All values are CSS hex strings suitable for inline `style` attributes.
//! AppThere uses a **Flat Design** aesthetic: solid fills, no gradients, no shadows.

// Token constants are consumed by multiple crates at different build stages.
#![allow(dead_code)]

// ── Surface colors ────────────────────────────────────────────────────────────

/// Pure white — document page surface and card backgrounds.
pub const COLOR_SURFACE_PAGE: &str = "#FFFFFF";

/// Mid-gray — application background behind document pages and cards.
pub const COLOR_SURFACE_BASE: &str = "#555555";

/// Darkest chrome surface — title bar and tab bar backgrounds.
pub const COLOR_SURFACE_CHROME: &str = "#1E1E1E";

/// Mid-dark chrome surface — sidebar, panel backgrounds.
pub const COLOR_SURFACE_1: &str = "#252525";

/// Raised chrome surface — card and input backgrounds in dark UI.
pub const COLOR_SURFACE_2: &str = "#2E2E2E";

/// Elevated chrome surface — dropdown, tooltip, popover backgrounds.
pub const COLOR_SURFACE_3: &str = "#383838";

// ── Border ────────────────────────────────────────────────────────────────────

/// Default border color for dividers and card outlines (light surfaces).
pub const COLOR_BORDER_DEFAULT: &str = "#E0E0E0";

/// Dark border for chrome surfaces (title bar, tab bar dividers).
pub const COLOR_BORDER_CHROME: &str = "#3A3A3A";

// ── Accent ────────────────────────────────────────────────────────────────────

/// Primary accent — buttons, active states, and key UI highlights.
pub const COLOR_ACCENT_PRIMARY: &str = "#3D7EFF";

/// Darker accent shown on hover over primary accent elements.
pub const COLOR_ACCENT_PRIMARY_HOVER: &str = "#3771E6";

// ── Text ──────────────────────────────────────────────────────────────────────

/// Primary text — headings and body copy (light surfaces).
pub const COLOR_TEXT_PRIMARY: &str = "#1A1A1A";

/// Secondary text — labels, captions, and metadata (light surfaces).
pub const COLOR_TEXT_SECONDARY: &str = "#6B6B6B";

/// Primary text on dark chrome surfaces.
pub const COLOR_TEXT_ON_CHROME: &str = "#E8E8E8";

/// Secondary text on dark chrome surfaces.
pub const COLOR_TEXT_ON_CHROME_SECONDARY: &str = "#888888";

/// Accent-colored text — collaborator count, active indicator labels.
pub const COLOR_TEXT_ACCENT: &str = "#4A9EFF";

// ── Tab bar chrome ────────────────────────────────────────────────────────────

/// Active tab background in the dark tab bar.
pub const COLOR_TAB_ACTIVE_BG: &str = "#2A2A2A";

/// 2 px bottom-border indicator on the active tab.
pub const COLOR_TAB_ACTIVE_INDICATOR: &str = "#4A9EFF";

/// Inactive tab background on hover.
pub const COLOR_TAB_INACTIVE_HOVER: &str = "#363636";

/// Amber — contextual ribbon tab accent (Format, Table, Image, etc.).
pub const COLOR_CONTEXTUAL_TAB: &str = "#E0A030";

// ── State overlays ────────────────────────────────────────────────────────────

/// CSS opacity value for disabled-state elements.
///
/// # COMPAT(dioxus-native)
///
/// `opacity` may not be supported in all Blitz versions — verify at runtime.
/// If unsupported, use [`COLOR_ICON_DISABLED`] for icon tint and
/// [`COLOR_TEXT_ON_CHROME_SECONDARY`] for text instead of opacity.
pub const OPACITY_DISABLED: &str = "0.35";

/// Explicit disabled tint for icon fills when opacity is not supported.
pub const COLOR_ICON_DISABLED: &str = "#555555";

// ── Document canvas ───────────────────────────────────────────────────────────

/// Light page surface within the dark shell canvas area.
pub const CANVAS_PAGE_BG: &str = "#FAFAFA";

/// Dark margin area surrounding pages inside the canvas.
pub const CANVAS_MARGIN_BG: &str = "#1C1C1C";

// ── Scrollbar ─────────────────────────────────────────────────────────────────

/// Scrollbar thumb — resting state (subtle semi-transparent light overlay).
///
/// # COMPAT(dioxus-native)
///
/// `scrollbar-color` is an unconfirmed CSS property in Blitz — verify at
/// runtime.  Falls back gracefully: if unsupported the platform default
/// scrollbar is shown.
pub const COLOR_SCROLLBAR_THUMB: &str = "rgba(255,255,255,0.22)";

/// Scrollbar thumb — hover / active state (more opaque, clearly visible).
///
/// # COMPAT(dioxus-native)
///
/// Same caveat as [`COLOR_SCROLLBAR_THUMB`].
pub const COLOR_SCROLLBAR_THUMB_HOVER: &str = "rgba(255,255,255,0.52)";

// ── Status: error ─────────────────────────────────────────────────────────────

/// Error banner background fill.
pub const COLOR_STATUS_ERROR_BG: &str = "#FFEBEE";

/// Error banner text and icon color.
pub const COLOR_STATUS_ERROR_TEXT: &str = "#C62828";

/// Error banner border color.
pub const COLOR_STATUS_ERROR_BORDER: &str = "#BB4433";
