// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Tier assignment policy for cached page textures.

use crate::key::CacheKey;
use crate::scroll::ScrollState;

/// Render quality tier for a cached page texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheTier {
    /// Full-resolution — pages visible or just off-screen.
    Hot,
    /// Half-resolution — pages within the warm margin.
    Warm,
    /// Quarter-resolution thumbnail — pages far from the viewport.
    Cold,
}

impl CacheTier {
    /// Linear scale factor applied to page dimensions at this tier.
    #[must_use]
    pub fn scale_factor(self) -> f32 {
        match self {
            CacheTier::Hot => 1.0,
            CacheTier::Warm => 0.5,
            CacheTier::Cold => 0.25,
        }
    }
}

/// Layout geometry of a single page in scroll-space.
#[derive(Debug, Clone, Copy)]
pub struct PageGeometry<K: CacheKey> {
    /// The cache key that identifies this page.
    pub index: K,
    /// Y coordinate of the top edge, in logical pixels.
    pub top_px: f64,
    /// Y coordinate of the bottom edge, in logical pixels.
    pub bottom_px: f64,
}

/// Assigns a [`CacheTier`] to `page` based on its distance from the scroll viewport.
///
/// Hot: overlaps the 2× viewport hot zone.
/// Warm: overlaps the hot zone ± 3× viewport margin.
/// Cold: beyond the warm zone.
#[must_use]
pub fn assign_tier<K: CacheKey>(page: &PageGeometry<K>, scroll: &ScrollState) -> CacheTier {
    let (hot_start, hot_end) = scroll.hot_range_px();

    if overlaps(page.top_px, page.bottom_px, hot_start, hot_end) {
        return CacheTier::Hot;
    }

    let margin = scroll.viewport_height_px * 3.0;
    let warm_start = hot_start - margin;
    let warm_end = hot_end + margin;

    if overlaps(page.top_px, page.bottom_px, warm_start, warm_end) {
        CacheTier::Warm
    } else {
        CacheTier::Cold
    }
}

fn overlaps(top: f64, bottom: f64, zone_start: f64, zone_end: f64) -> bool {
    bottom > zone_start && top < zone_end
}
