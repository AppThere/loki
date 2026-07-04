// SPDX-License-Identifier: Apache-2.0

//! Breakpoint-driven posture for the style editor panel (Spec 05 M7 / §11).
//!
//! At Expanded/Medium the panel is a wide, side-by-side surface (tree list +
//! form + provenance + family inspectors). At Compact (< 600 px, Spec 03) there
//! is no room beside the document, so it becomes a **full-width stacked sheet**
//! with a segmented Edit/Inspect switch and ≥44 px touch targets. This is a pure
//! `for_breakpoint` mapping so the posture is testable without a real window,
//! mirroring [`appthere_ui::PanelPosture`].

use appthere_ui::responsive::Breakpoint;
use appthere_ui::tokens;

/// Compact sheet height — taller than the Expanded side panel because the body
/// stacks vertically (§11 full-surface posture).
pub(super) const STYLE_PANEL_COMPACT_HEIGHT_PX: f32 = 520.0;

/// The posture the style panel adopts for a size class.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct StylePanelPosture {
    /// Stack the body vertically (Compact sheet) instead of side-by-side columns.
    pub stack: bool,
    /// Each section fills the panel width instead of its fixed column width.
    pub full_width: bool,
    /// Minimum interactive height (logical px) for nav controls; 0 = natural.
    pub min_touch_px: f32,
    /// The panel's outer height in CSS pixels.
    pub height_px: f32,
}

impl StylePanelPosture {
    /// The posture for a size class.
    #[must_use]
    pub fn for_breakpoint(bp: Breakpoint) -> Self {
        if bp.is_compact() {
            Self {
                stack: true,
                full_width: true,
                min_touch_px: tokens::TOUCH_MIN,
                height_px: STYLE_PANEL_COMPACT_HEIGHT_PX,
            }
        } else {
            Self {
                stack: false,
                full_width: false,
                min_touch_px: 0.0,
                height_px: super::STYLE_EDITOR_HEIGHT_PX,
            }
        }
    }

    /// The body's `flex-direction` (`column` when stacked, else `row`).
    #[must_use]
    pub fn body_direction(self) -> &'static str {
        if self.stack { "column" } else { "row" }
    }

    /// The CSS `width` for a section that is fixed-width at Expanded: `100%` when
    /// stacked, else `expanded_px`.
    #[must_use]
    pub fn section_width(self, expanded_px: f32) -> String {
        if self.full_width {
            "100%".to_string()
        } else {
            format!("{expanded_px}px")
        }
    }

    /// A `min-height: Npx;` fragment for a touch target, empty when natural.
    #[must_use]
    pub fn touch_min_css(self) -> String {
        if self.min_touch_px > 0.0 {
            format!("min-height: {}px;", self.min_touch_px)
        } else {
            String::new()
        }
    }
}

#[cfg(test)]
#[path = "posture_tests.rs"]
mod tests;
