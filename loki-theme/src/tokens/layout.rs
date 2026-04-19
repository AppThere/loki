// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Layout design tokens — toolbar heights, page dimensions, and breakpoints.

// Token constants may not all be referenced in every build state.
#![allow(dead_code)]

// ── Toolbar heights (px) ──────────────────────────────────────────────────────

/// Height of the top toolbar in the editor shell.
pub const TOOLBAR_HEIGHT_TOP: f32 = 48.0;

/// Height of the bottom status bar in the editor shell.
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
/// Used to derive the text content width: `PAGE_WIDTH_PX - 2 × PAGE_MARGIN_PX = 650 px`.
/// The layout engine reads margins from the document's own [`PageLayout`]; this
/// constant is provided for UI components that need to reflect the margin visually
/// (e.g., ruler, margin handles, scroll-gutter calculations).
pub const PAGE_MARGIN_PX: f32 = 72.0;

// ── Responsive breakpoints ────────────────────────────────────────────────────

/// Viewport width above which the UI switches to the desktop two-column layout.
pub const BREAKPOINT_DESKTOP_PX: f32 = 768.0;

/// Maximum width for primary action buttons on desktop (centered, fixed width).
pub const BUTTON_WIDTH_DESKTOP_MAX: f32 = 320.0;
