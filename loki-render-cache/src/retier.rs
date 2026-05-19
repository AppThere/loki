// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Tier reassignment logic.
//!
//! [`PageCache::retier`] decides what must be re-rendered and what can be
//! downsampled (tier demotion) based on the current scroll position.
//! GPU memory is now managed by Blitz via `CustomPaintSource`, so byte-budget
//! eviction is no longer performed here.

use crate::key::CacheKey;
use crate::page_cache::PageCache;
use crate::tier_policy::{CacheTier, assign_tier};
use crate::{PageGeometry, ScrollState};

/// The outcome of a single [`PageCache::retier`] call.
#[derive(Debug)]
pub struct RetierResult<K: CacheKey> {
    /// Pages whose tier has changed to a finer resolution (or that are not yet
    /// cached). `LokiPageSource` will re-render these at the new tier.
    pub rerender: Vec<(K, CacheTier)>,
    /// Pages whose tier has been demoted (e.g. Hot→Warm). `LokiPageSource`
    /// will re-render at lower quality on the next frame.
    pub downsample: Vec<K>,
}

impl<K: CacheKey> Default for RetierResult<K> {
    fn default() -> Self {
        Self {
            rerender: Vec::new(),
            downsample: Vec::new(),
        }
    }
}

/// Numeric quality rank: higher = finer resolution.
fn quality(tier: CacheTier) -> u8 {
    match tier {
        CacheTier::Hot => 2,
        CacheTier::Warm => 1,
        CacheTier::Cold => 0,
    }
}

impl<K: CacheKey> PageCache<K> {
    /// Reassigns tiers for all `pages` based on `scroll`.
    ///
    /// Call when [`ScrollState::tick`] returns `true` (→ Settling).
    ///
    /// Not cached → `rerender`; same tier clean → skip; same tier dirty →
    /// `rerender`; finer tier (e.g. Cold→Hot) → `rerender`; coarser tier
    /// (e.g. Hot→Warm) → `downsample`.
    pub fn retier(
        &mut self,
        pages: &[PageGeometry<K>],
        scroll: &ScrollState,
    ) -> RetierResult<K> {
        let mut result = RetierResult::default();

        for page_geom in pages {
            let idx = page_geom.index;
            let new_tier = assign_tier(page_geom, scroll);

            match self.pages.get_mut(&idx) {
                None => {
                    result.rerender.push((idx, new_tier));
                }
                Some(cached) => {
                    let old_quality = quality(cached.tier);
                    let new_quality = quality(new_tier);

                    if new_quality > old_quality {
                        cached.tier = new_tier;
                        cached.dirty = true;
                        result.rerender.push((idx, new_tier));
                    } else if new_quality < old_quality {
                        cached.tier = new_tier;
                        result.downsample.push(idx);
                    } else if cached.dirty {
                        result.rerender.push((idx, cached.tier));
                    }
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use crate::scroll_state::{SETTLE_DURATION, ScrollState};
    use crate::{CacheTier, PageCache, PageGeometry, PageIndex};

    /// Scroll state with the viewport so far down that all test pages
    /// (placed near y=0) fall into the Cold tier.
    fn cold_scroll() -> ScrollState {
        let mut s = ScrollState::new(800.0);
        s.viewport_top_px = 100_000.0;
        s
    }

    /// Scroll state with the viewport centred on y=0..200 so those pages
    /// are in the Hot zone.
    fn hot_scroll() -> ScrollState {
        let mut s = ScrollState::new(800.0);
        s.viewport_top_px = 0.0;
        s
    }

    fn page(index: u32) -> PageGeometry<PageIndex> {
        let top = f64::from(index) * 300.0;
        PageGeometry {
            index: PageIndex(index),
            top_px: top,
            bottom_px: top + 200.0,
        }
    }

    #[test]
    fn uncached_page_appears_in_rerender() {
        let mut cache = PageCache::new();
        let result = cache.retier(&[page(0)], &hot_scroll());
        assert_eq!(result.rerender.len(), 1);
        assert_eq!(result.rerender[0].0, PageIndex(0));
        assert!(result.downsample.is_empty());
    }

    #[test]
    fn cached_clean_same_tier_produces_no_action() {
        let mut cache = PageCache::new();
        cache.insert(PageIndex(0), CacheTier::Hot);
        let result = cache.retier(&[page(0)], &hot_scroll());
        assert!(result.rerender.is_empty());
        assert!(result.downsample.is_empty());
    }

    #[test]
    fn cached_dirty_same_tier_appears_in_rerender() {
        let mut cache = PageCache::new();
        cache.insert(PageIndex(0), CacheTier::Hot);
        cache.mark_dirty(PageIndex(0));
        let result = cache.retier(&[page(0)], &hot_scroll());
        assert_eq!(result.rerender.len(), 1);
        assert_eq!(result.rerender[0].0, PageIndex(0));
        assert!(result.downsample.is_empty());
    }

    #[test]
    fn cold_to_hot_promotion_appears_in_rerender_not_downsample() {
        let mut cache = PageCache::new();
        cache.insert(PageIndex(0), CacheTier::Cold);
        let result = cache.retier(&[page(0)], &hot_scroll());
        assert_eq!(result.rerender.len(), 1);
        assert_eq!(result.rerender[0], (PageIndex(0), CacheTier::Hot));
        assert!(result.downsample.is_empty());
    }

    #[test]
    fn hot_to_warm_demotion_appears_in_downsample_not_rerender() {
        let mut cache = PageCache::new();
        cache.insert(PageIndex(0), CacheTier::Hot);
        let mut scroll = ScrollState::new(800.0);
        scroll.viewport_top_px = 2000.0; // page 0 → Warm
        let result = cache.retier(&[page(0)], &scroll);
        assert_eq!(result.downsample.len(), 1);
        assert_eq!(result.downsample[0], PageIndex(0));
        assert!(result.rerender.is_empty());
    }

    #[test]
    fn repeated_retier_on_same_coarser_tier_produces_no_action() {
        let mut cache = PageCache::new();
        cache.insert(PageIndex(0), CacheTier::Hot);
        let mut scroll = ScrollState::new(800.0);
        scroll.viewport_top_px = 2000.0; // page 0 → Warm

        let r1 = cache.retier(&[page(0)], &scroll);
        assert_eq!(r1.downsample.len(), 1);

        let r2 = cache.retier(&[page(0)], &scroll);
        assert!(r2.rerender.is_empty());
        assert!(r2.downsample.is_empty());
    }

    #[test]
    fn retier_called_on_settling_transition() {
        let mut scroll = ScrollState::new(800.0);
        let t0 = Instant::now();
        scroll.on_scroll(100.0);

        let settled = scroll.tick(t0 + SETTLE_DURATION + std::time::Duration::from_millis(10));
        assert!(settled);

        let mut cache = PageCache::new();
        let result = cache.retier(&[page(0)], &scroll);
        assert!(!result.rerender.is_empty());
    }

    #[test]
    fn cold_pages_no_longer_evicted() {
        // With byte-budget eviction removed, Cold pages are never evicted
        // by retier — GPU memory is managed by Blitz.
        let mut cache = PageCache::new();
        for i in 0..4u32 {
            cache.insert(PageIndex(i), CacheTier::Cold);
        }
        let pages: Vec<_> = (0..4).map(page).collect();
        let result = cache.retier(&pages, &cold_scroll());
        // No evictions — all pages stay cached.
        assert_eq!(cache.pages.len(), 4);
        let _ = result; // downsample/rerender counts may vary
    }
}
