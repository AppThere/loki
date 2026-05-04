// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Tier assignment policy for cached page textures.
//!
//! The three tiers trade render quality for memory:
//!
//! | Tier | Scale factor | Texture size (1080 p page) |
//! |------|-------------|---------------------------|
//! | Hot  | 1.0         | ~12 MB                    |
//! | Warm | 0.5         | ~3 MB                     |
//! | Cold | 0.25        | ~0.75 MB                  |

use crate::scroll_state::ScrollState;

/// The render quality tier for a cached page texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheTier {
    /// Full-resolution texture — pages currently visible or just off-screen.
    Hot,
    /// Half-resolution texture — pages within the warm margin.
    Warm,
    /// Quarter-resolution thumbnail — pages far from the viewport.
    Cold,
}

impl CacheTier {
    /// Linear scale factor applied to page dimensions when rendering at this
    /// tier. The resulting texture has `(scale × width) × (scale × height)`
    /// pixels.
    #[must_use]
    pub fn scale_factor(self) -> f32 {
        match self {
            CacheTier::Hot => 1.0,
            CacheTier::Warm => 0.5,
            CacheTier::Cold => 0.25,
        }
    }
}

/// The layout geometry of a single document page in scroll-space.
#[derive(Debug, Clone, Copy)]
pub struct PageGeometry {
    /// Zero-based page index within the document.
    pub index: u32,
    /// Y coordinate of the top edge of the page, in logical pixels.
    pub top_px: f64,
    /// Y coordinate of the bottom edge of the page, in logical pixels.
    pub bottom_px: f64,
}

/// Assigns a [`CacheTier`] to `page` based on its distance from the visible
/// area described by `scroll`.
///
/// # Tier boundaries
///
/// - **Hot**: the page overlaps the *hot zone* — a band of `2× viewport height`
///   centred on the visible area (see [`ScrollState::hot_range_px`]).
/// - **Warm**: the page overlaps the *warm zone* — the hot zone extended by
///   `3× viewport height` on each side.
/// - **Cold**: anything beyond the warm zone.
///
/// A page that partially overlaps a zone boundary is assigned the higher-
/// priority tier (Hot beats Warm beats Cold).
#[must_use]
pub fn assign_tier(page: &PageGeometry, scroll: &ScrollState) -> CacheTier {
    let (hot_start, hot_end) = scroll.hot_range_px();

    if overlaps(page, hot_start, hot_end) {
        return CacheTier::Hot;
    }

    let margin = scroll.viewport_height_px * 3.0;
    let warm_start = hot_start - margin;
    let warm_end = hot_end + margin;

    if overlaps(page, warm_start, warm_end) {
        CacheTier::Warm
    } else {
        CacheTier::Cold
    }
}

/// Returns `true` when `page` has any overlap with the interval
/// `[zone_start, zone_end)`.
fn overlaps(page: &PageGeometry, zone_start: f64, zone_end: f64) -> bool {
    page.bottom_px > zone_start && page.top_px < zone_end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scroll_state::ScrollState;

    /// Build a scroll state with the viewport at the given top offset.
    fn scroll_at(top: f64, height: f64) -> ScrollState {
        let mut s = ScrollState::new(height);
        s.viewport_top_px = top;
        s
    }

    /// Build a page spanning `[top, bottom)`.
    fn page(top: f64, bottom: f64) -> PageGeometry {
        PageGeometry { index: 0, top_px: top, bottom_px: bottom }
    }

    // viewport_top = 1000, viewport_height = 800
    // hot zone:  (1000 - 400, 1000 + 1200) = (600, 2200)
    // warm zone: (600 - 2400, 2200 + 2400) = (-1800, 4600)

    #[test]
    fn page_fully_inside_hot_zone_is_hot() {
        let scroll = scroll_at(1000.0, 800.0);
        // page [800, 1200] — fully inside hot zone [600, 2200]
        assert_eq!(assign_tier(&page(800.0, 1200.0), &scroll), CacheTier::Hot);
    }

    #[test]
    fn page_partially_overlapping_hot_zone_boundary_is_hot() {
        let scroll = scroll_at(1000.0, 800.0);
        // page [400, 700] — bottom (700) > hot_start (600), so overlaps
        assert_eq!(assign_tier(&page(400.0, 700.0), &scroll), CacheTier::Hot);
    }

    #[test]
    fn page_just_outside_hot_zone_inside_warm_is_warm() {
        let scroll = scroll_at(1000.0, 800.0);
        // page [100, 590] — does NOT overlap hot [600, 2200],
        // but overlaps warm [-1800, 4600]
        assert_eq!(assign_tier(&page(100.0, 590.0), &scroll), CacheTier::Warm);
    }

    #[test]
    fn page_beyond_warm_margin_is_cold() {
        let scroll = scroll_at(1000.0, 800.0);
        // warm_start = 600 - 2400 = -1800; page [-2500, -1850] is outside
        assert_eq!(assign_tier(&page(-2500.0, -1850.0), &scroll), CacheTier::Cold);
    }

    #[test]
    fn page_above_scroll_follows_same_tier_rules() {
        // Viewport at 5000, looking downward.
        // hot zone: (5000 - 400, 5000 + 1200) = (4600, 6200)
        // warm zone: (4600 - 2400, 6200 + 2400) = (2200, 8600)
        let scroll = scroll_at(5000.0, 800.0);

        // Already-scrolled-past page at [300, 500] — before warm zone
        assert_eq!(assign_tier(&page(300.0, 500.0), &scroll), CacheTier::Cold);

        // Page at [2500, 3000] — inside warm zone
        assert_eq!(assign_tier(&page(2500.0, 3000.0), &scroll), CacheTier::Warm);

        // Page at [4700, 5200] — inside hot zone
        assert_eq!(assign_tier(&page(4700.0, 5200.0), &scroll), CacheTier::Hot);
    }

    // ── CacheTier::scale_factor ───────────────────────────────────────────────

    #[test]
    fn scale_factors_are_ordered_hot_warm_cold() {
        assert!(CacheTier::Hot.scale_factor() > CacheTier::Warm.scale_factor());
        assert!(CacheTier::Warm.scale_factor() > CacheTier::Cold.scale_factor());
        assert_eq!(CacheTier::Hot.scale_factor(), 1.0_f32);
    }
}
