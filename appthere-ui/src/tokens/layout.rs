// SPDX-License-Identifier: Apache-2.0

//! Layout design tokens — shell chrome heights, page dimensions, and breakpoints.

// Token constants may not all be referenced in every build stage.
#![allow(dead_code)]

// ── Shell chrome heights (px) ─────────────────────────────────────────────────

/// Height of the title bar on Windows and Linux.
pub const TITLE_BAR_HEIGHT_DEFAULT: f32 = 36.0;

/// Height of the title bar on macOS (slightly shorter — traffic lights fit here).
pub const TITLE_BAR_HEIGHT_MACOS: f32 = 40.0;

/// Height of the document tab bar.
pub const TAB_BAR_HEIGHT: f32 = 40.0;

/// Height of the top toolbar in the editor shell.
///
/// Retained for backward compatibility with existing editor layout calculations.
pub const TOOLBAR_HEIGHT_TOP: f32 = 48.0;

/// Height of the bottom status bar.
pub const STATUS_BAR_HEIGHT: f32 = 24.0;

/// Height of the bottom toolbar / status bar in the editor shell.
///
/// Retained for backward compatibility; prefer [`STATUS_BAR_HEIGHT`] in new code.
pub const TOOLBAR_HEIGHT_BOTTOM: f32 = 36.0;

// ── Document page dimensions (A4 at 96 dpi) ──────────────────────────────────

/// A4 page width in CSS pixels at 96 dpi equivalent (210 mm → ~794 px).
pub const PAGE_WIDTH_PX: f32 = 794.0;

/// A4 page height in CSS pixels at 96 dpi equivalent (297 mm → ~1123 px).
pub const PAGE_HEIGHT_PX: f32 = 1123.0;

/// Vertical gap between stacked pages in the editor scroll canvas (px).
pub const PAGE_GAP_PX: f32 = 24.0;

/// Standard document page margin in CSS pixels (≈ 1 inch at 96 dpi).
///
/// Used to derive text content width: `PAGE_WIDTH_PX - 2 × PAGE_MARGIN_PX = 650 px`.
/// The layout engine reads margins from the document's own `PageLayout`; this
/// constant is provided for UI components that need to reflect the margin visually
/// (e.g., ruler, margin handles, scroll-gutter calculations).
pub const PAGE_MARGIN_PX: f32 = 72.0;

// ── Ribbon heights (px) ───────────────────────────────────────────────────────

/// Height of the ribbon tab strip (the row of tab labels: Home, Insert, etc.).
pub const RIBBON_TAB_STRIP_HEIGHT: f32 = 36.0;

/// Height of the ribbon content row (the row of buttons below the tab strip).
pub const RIBBON_CONTENT_HEIGHT: f32 = 60.0;

/// Total ribbon height: tab strip + content row.
/// Used by Shell to reserve space and by canvas height calculations.
pub const RIBBON_TOTAL_HEIGHT: f32 = RIBBON_TAB_STRIP_HEIGHT + RIBBON_CONTENT_HEIGHT;

// ── Responsive breakpoints ────────────────────────────────────────────────────

/// Viewport width above which the UI switches to the desktop two-column layout.
pub const BREAKPOINT_DESKTOP_PX: f32 = 768.0;

/// Maximum width for primary action buttons on desktop (centered, fixed width).
pub const BUTTON_WIDTH_DESKTOP_MAX: f32 = 320.0;
