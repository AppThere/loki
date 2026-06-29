// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Semantic window-size classification (Spec 03 §5).

use crate::tokens::layout::{BREAKPOINT_COMPACT_MAX_PX, BREAKPOINT_EXPANDED_MIN_PX};

/// A semantic window-size class, derived from the measured viewport width.
///
/// Breakpoints are **size classes, not device names** (Spec 03 D4): two windows
/// of equal width behave identically regardless of hardware, so tablets,
/// split-screen windows, and large landscape phones classify by width alone. The
/// variants are ordered `Compact < Medium < Expanded`, so a "`>= Medium`" check
/// is a plain comparison.
///
/// This is the **single source of truth** for any responsive behaviour that must
/// be testable without a real window (Spec 03 D1) — panel collapse, renderer
/// posture, chrome density. It is derived state: compute it once from the shared
/// [`Viewport`](crate::responsive::Viewport) and read it via
/// [`use_breakpoint`](crate::responsive::use_breakpoint), never re-measure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Breakpoint {
    /// `< 600` px — single column, touch-first chrome, non-paginated, stacked
    /// panels.
    Compact,
    /// `600–1024` px — transitional; page-fit decides the renderer, panels may
    /// overlay.
    Medium,
    /// `>= 1024` px — paginated, full chrome, side-by-side panels.
    Expanded,
}

impl Breakpoint {
    /// Classifies a measured width (CSS px) into a size class.
    ///
    /// An unmeasured / zero width classifies as [`Breakpoint::Compact`]
    /// (mobile-first; the first measured frame corrects it).
    #[must_use]
    pub fn from_width(width_px: f32) -> Self {
        if width_px < BREAKPOINT_COMPACT_MAX_PX {
            Breakpoint::Compact
        } else if width_px < BREAKPOINT_EXPANDED_MIN_PX {
            Breakpoint::Medium
        } else {
            Breakpoint::Expanded
        }
    }

    /// `true` for [`Breakpoint::Compact`].
    #[must_use]
    pub fn is_compact(self) -> bool {
        self == Breakpoint::Compact
    }

    /// `true` for [`Breakpoint::Medium`].
    #[must_use]
    pub fn is_medium(self) -> bool {
        self == Breakpoint::Medium
    }

    /// `true` for [`Breakpoint::Expanded`].
    #[must_use]
    pub fn is_expanded(self) -> bool {
        self == Breakpoint::Expanded
    }

    /// `true` where chrome should adopt a touch-first posture (≥44 px targets,
    /// stacked layouts). Currently the Compact class.
    #[must_use]
    pub fn is_touch_first(self) -> bool {
        self == Breakpoint::Compact
    }
}

#[cfg(test)]
#[path = "breakpoint_tests.rs"]
mod tests;
