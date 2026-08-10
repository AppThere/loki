// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Breakpoint-driven posture for a tabbed modal dialog.
//!
//! At Expanded the dialog is a centred card of fixed width with a side-docked
//! preview rail and a horizontal footer. At Medium it keeps the card shape but
//! takes the available width up to a cap, and the preview rail collapses into
//! the body. At Compact (< 600 px, Spec 03) there is no room for a card at all,
//! so it becomes a **full-screen sheet**: no backdrop inset, no corner radius,
//! ≥44 px controls, and a stacked footer with the primary action first.
//!
//! This mirrors [`crate::PanelPosture`] and the style panel's own
//! `StylePanelPosture`: a pure `for_breakpoint` mapping, so every layout claim
//! below is testable without a real window (Spec 03 D1).

use crate::responsive::Breakpoint;
use crate::tokens;

/// The width class a dialog card adopts. Wide dialogs carry a preview rail;
/// narrow ones are a single body column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogWidth {
    /// A tabbed editor with a preview rail (paragraph / page / publish).
    Wide,
    /// A single-column dialog (insert link, insert table).
    Narrow,
}

impl DialogWidth {
    /// The Expanded fixed card width in CSS px.
    #[must_use]
    pub fn expanded_px(self) -> f32 {
        match self {
            DialogWidth::Wide => tokens::DIALOG_WIDTH_WIDE_PX,
            DialogWidth::Narrow => tokens::DIALOG_WIDTH_NARROW_PX,
        }
    }
}

/// The posture a dialog adopts for a size class.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DialogPosture {
    /// Fill the whole frame as a sheet instead of floating as a centred card.
    pub full_screen: bool,
    /// Dock the preview beside the form instead of collapsing it into the body.
    pub preview_docked: bool,
    /// Stack footer actions full-width (primary first) instead of a trailing row.
    pub stack_footer: bool,
    /// Minimum interactive height (CSS px) for controls; 0 = natural density.
    pub min_touch_px: f32,
}

impl DialogPosture {
    /// The posture for a size class.
    #[must_use]
    pub fn for_breakpoint(bp: Breakpoint) -> Self {
        match bp {
            Breakpoint::Compact => Self {
                full_screen: true,
                preview_docked: false,
                stack_footer: true,
                min_touch_px: tokens::TOUCH_MIN,
            },
            // The preview rail docks only at Expanded: below 1024 px the form
            // column would lose more width than the preview gains (note 05).
            Breakpoint::Medium => Self {
                full_screen: false,
                preview_docked: false,
                stack_footer: false,
                min_touch_px: 0.0,
            },
            Breakpoint::Expanded => Self {
                full_screen: false,
                preview_docked: true,
                stack_footer: false,
                min_touch_px: 0.0,
            },
        }
    }

    /// The card's CSS `width` / `max-width` pair for a width class.
    ///
    /// Compact fills the frame; Medium takes the available width up to
    /// [`tokens::DIALOG_WIDTH_MEDIUM_MAX_PX`]; Expanded pins the fixed width.
    #[must_use]
    pub fn card_width_css(self, width: DialogWidth) -> String {
        if self.full_screen {
            return "width: 100%; height: 100%;".to_string();
        }
        if self.preview_docked {
            format!("width: {}px; max-width: 100%;", width.expanded_px())
        } else {
            format!(
                "width: 100%; max-width: {}px;",
                tokens::DIALOG_WIDTH_MEDIUM_MAX_PX
            )
        }
    }

    /// The card's `border-radius` in CSS px — square at Compact, where the sheet
    /// meets the frame edges and a radius would show the backdrop through the
    /// corners.
    #[must_use]
    pub fn card_radius_px(self) -> f32 {
        if self.full_screen {
            0.0
        } else {
            tokens::RADIUS_LG
        }
    }

    /// A `min-height: Npx;` fragment for a touch target, empty at pointer
    /// density.
    #[must_use]
    pub fn touch_min_css(self) -> String {
        if self.min_touch_px > 0.0 {
            format!("min-height: {}px;", self.min_touch_px)
        } else {
            String::new()
        }
    }

    /// The footer's `flex-direction`: stacked at Compact, a row elsewhere.
    #[must_use]
    pub fn footer_direction(self) -> &'static str {
        if self.stack_footer {
            "column"
        } else {
            "row"
        }
    }
}

#[cfg(test)]
#[path = "posture_tests.rs"]
mod tests;
