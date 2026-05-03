// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Tier reassignment and Cold-tier eviction logic.
//!
//! [`PageCache::retier`] is the heart of the cache policy: it decides what
//! must be re-rendered, what can be cheaply downsampled, and what must be
//! evicted to stay within the Cold-tier byte budget.

use crate::page_cache::PageCache;
use crate::tier_policy::{CacheTier, assign_tier};
use crate::{PageGeometry, PageIndex, ScrollState};

/// The outcome of a single [`PageCache::retier`] call.
#[derive(Debug, Default)]
pub struct RetierResult {
    /// Pages that must be fully re-rendered at the given tier by the Vello
    /// pipeline. Covers uncached pages, dirty pages, and tier upgrades
    /// (Cold→Warm, Warm→Hot, etc.).
    pub rerender: Vec<(PageIndex, CacheTier)>,
    /// Pages whose existing texture can be downsampled (blit + scale) rather
    /// than re-rendered from scratch. Covers tier demotions (Hot→Warm,
    /// Warm→Cold, Hot→Cold).
    pub downsample: Vec<PageIndex>,
    /// Pages evicted from the Cold tier to keep `cold_bytes` within
    /// [`PageCache::cold_budget_bytes`]. Their textures have been dropped.
    pub evicted: Vec<PageIndex>,
}

/// Numeric quality rank: higher = finer resolution.
fn quality(tier: CacheTier) -> u8 {
    match tier {
        CacheTier::Hot => 2,
        CacheTier::Warm => 1,
        CacheTier::Cold => 0,
    }
}

impl PageCache {
    /// Reassigns tiers for all `pages` based on `scroll`, then evicts Cold
    /// overflow. Call when [`ScrollState::tick`] returns `true` (→ Settling).
    ///
    /// Not cached → `rerender`; same tier clean → skip; same tier dirty →
    /// `rerender`; finer tier (e.g. Cold→Hot) → `rerender`; coarser tier
    /// (e.g. Hot→Warm) → `downsample`.
    pub fn retier(
        &mut self,
        pages: &[PageGeometry],
        scroll: &ScrollState,
    ) -> RetierResult {
        let mut result = RetierResult::default();

        for page_geom in pages {
            let idx = PageIndex(page_geom.index);
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
                        // Update tier now to prevent repeat demotion next call.
                        cached.tier = new_tier;
                        result.downsample.push(idx);
                    } else if cached.dirty {
                        result.rerender.push((idx, cached.tier));
                    }
                }
            }
        }

        result.evicted = self.evict_cold_overflow();
        result
    }

    /// Removes the oldest Cold-tier pages until `cold_bytes()` is within
    /// [`PageCache::cold_budget_bytes`]. Returns the evicted page indices.
    fn evict_cold_overflow(&mut self) -> Vec<PageIndex> {
        if self.cold_bytes() <= self.cold_budget_bytes {
            return Vec::new();
        }

        // Collect Cold entries sorted oldest-first by LRU clock.
        let mut cold: Vec<(PageIndex, u64)> = self
            .pages
            .iter()
            .filter(|(_, p)| p.tier == CacheTier::Cold)
            .map(|(&idx, p)| (idx, p.last_access))
            .collect();
        cold.sort_unstable_by_key(|&(_, access)| access);

        let mut evicted = Vec::new();
        for (idx, _) in cold {
            if self.cold_bytes() <= self.cold_budget_bytes {
                break;
            }
            self.pages.remove(&idx);
            evicted.push(idx);
        }
        evicted
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use crate::mock_texture::MockTexture;
    use crate::scroll_state::{SETTLE_DURATION, ScrollState};
    use crate::{CacheTier, PageCache, PageGeometry, PageIndex};

    fn tex(w: u32, h: u32) -> MockTexture {
        MockTexture { width: w, height: h }
    }

    /// Scroll state with the viewport so far down that all test pages
    /// (placed near y=0) fall into the Cold tier.
    fn cold_scroll() -> ScrollState {
        // viewport at y=100_000, height=800
        // hot zone:  (99_600, 101_200)
        // warm zone: (97_200, 103_600)
        // pages at y=0..200 are Cold
        let mut s = ScrollState::new(800.0);
        s.viewport_top_px = 100_000.0;
        s
    }

    /// Scroll state with the viewport centred on y=0..200 so those pages
    /// are in the Hot zone.
    fn hot_scroll() -> ScrollState {
        // viewport at y=0, height=800  →  hot zone: (-400, 1200)
        let mut s = ScrollState::new(800.0);
        s.viewport_top_px = 0.0;
        s
    }

    fn page(index: u32) -> PageGeometry {
        // A thin page at y = index * 300, height 200
        let top = f64::from(index) * 300.0;
        PageGeometry { index, top_px: top, bottom_px: top + 200.0 }
    }

    // ── basic retier cases ────────────────────────────────────────────────────

    #[test]
    fn uncached_page_appears_in_rerender() {
        let mut cache = PageCache::new(u64::MAX);
        let scroll = hot_scroll();
        let result = cache.retier(&[page(0)], &scroll);
        assert_eq!(result.rerender.len(), 1);
        assert_eq!(result.rerender[0].0, PageIndex(0));
        assert!(result.downsample.is_empty());
        assert!(result.evicted.is_empty());
    }

    #[test]
    fn cached_clean_same_tier_produces_no_action() {
        let mut cache = PageCache::new(u64::MAX);
        // Insert page 0 as Hot; viewport keeps it Hot.
        cache.insert(PageIndex(0), CacheTier::Hot, tex(10, 10));
        let scroll = hot_scroll();
        let result = cache.retier(&[page(0)], &scroll);
        assert!(result.rerender.is_empty());
        assert!(result.downsample.is_empty());
        assert!(result.evicted.is_empty());
    }

    #[test]
    fn cached_dirty_same_tier_appears_in_rerender() {
        let mut cache = PageCache::new(u64::MAX);
        cache.insert(PageIndex(0), CacheTier::Hot, tex(10, 10));
        cache.mark_dirty(PageIndex(0));
        let scroll = hot_scroll();
        let result = cache.retier(&[page(0)], &scroll);
        assert_eq!(result.rerender.len(), 1);
        assert_eq!(result.rerender[0].0, PageIndex(0));
        assert!(result.downsample.is_empty());
    }

    #[test]
    fn cold_to_hot_promotion_appears_in_rerender_not_downsample() {
        let mut cache = PageCache::new(u64::MAX);
        // Page 0 was cached as Cold.
        cache.insert(PageIndex(0), CacheTier::Cold, tex(10, 10));
        // Scroll so page 0 is now Hot.
        let scroll = hot_scroll();
        let result = cache.retier(&[page(0)], &scroll);
        assert_eq!(result.rerender.len(), 1);
        assert_eq!(result.rerender[0], (PageIndex(0), CacheTier::Hot));
        assert!(result.downsample.is_empty());
    }

    #[test]
    fn hot_to_warm_demotion_appears_in_downsample_not_rerender() {
        let mut cache = PageCache::new(u64::MAX);
        cache.insert(PageIndex(0), CacheTier::Hot, tex(10, 10));
        // Build a scroll that puts page 0 into Warm.
        // hot zone for this scroll:  (-400, 1200); warm zone: (-2800, 3600)
        // page 0 is at y=0..200 → it overlaps the hot zone → Hot.
        // We need page 0 to land in Warm. Put viewport far enough:
        // viewport_top = 2000, height = 800
        // hot zone: (1600, 3200)   warm zone: (-800, 5600)
        // page 0 (y=0..200) does not overlap hot [1600,3200] but overlaps warm [-800,5600] → Warm ✓
        let mut scroll = ScrollState::new(800.0);
        scroll.viewport_top_px = 2000.0;
        let result = cache.retier(&[page(0)], &scroll);
        assert_eq!(result.downsample.len(), 1);
        assert_eq!(result.downsample[0], PageIndex(0));
        assert!(result.rerender.is_empty());
    }

    // ── eviction ─────────────────────────────────────────────────────────────

    #[test]
    fn cold_overflow_evicts_oldest_pages_first() {
        // Budget: 800 bytes. Each Cold page is 10×10×4 = 400 B.
        // Insert 4 Cold pages (1600 B total). After retier that keeps them
        // all Cold, should evict oldest 2 to get to ≤ 800 B.
        let budget = 800_u64;
        let mut cache = PageCache::new(budget);

        // Insert pages 0..3 in order — page 0 is oldest (last_access=1).
        for i in 0..4u32 {
            cache.insert(PageIndex(i), CacheTier::Cold, tex(10, 10));
        }
        assert_eq!(cache.cold_bytes(), 1600);

        // All pages at y=0..200, 300..500, 600..800, 900..1100
        // cold_scroll puts viewport at y=100_000, so all four are Cold.
        let pages: Vec<PageGeometry> = (0..4).map(page).collect();
        let result = cache.retier(&pages, &cold_scroll());

        // 2 oldest (indices 0 and 1) should be evicted to reach ≤ 800 B.
        assert_eq!(result.evicted.len(), 2, "expected 2 evictions");
        assert!(result.evicted.contains(&PageIndex(0)));
        assert!(result.evicted.contains(&PageIndex(1)));
        assert!(cache.cold_bytes() <= budget);
    }

    #[test]
    fn no_eviction_when_within_budget() {
        let mut cache = PageCache::new(u64::MAX);
        cache.insert(PageIndex(0), CacheTier::Cold, tex(10, 10));
        let result = cache.retier(&[page(0)], &cold_scroll());
        assert!(result.evicted.is_empty());
    }

    // ── second retier doesn't re-trigger demotion ─────────────────────────────

    #[test]
    fn repeated_retier_on_same_coarser_tier_produces_no_action() {
        let mut cache = PageCache::new(u64::MAX);
        cache.insert(PageIndex(0), CacheTier::Hot, tex(10, 10));
        let mut scroll = ScrollState::new(800.0);
        scroll.viewport_top_px = 2000.0; // page 0 → Warm

        // First call demotes Hot → Warm.
        let r1 = cache.retier(&[page(0)], &scroll);
        assert_eq!(r1.downsample.len(), 1);

        // Second call: tier is now Warm, assign_tier still returns Warm → no action.
        let r2 = cache.retier(&[page(0)], &scroll);
        assert!(r2.rerender.is_empty());
        assert!(r2.downsample.is_empty());
    }

    // ── tick integration — retier is called when tick returns true ────────────

    #[test]
    fn retier_called_on_settling_transition() {
        let mut scroll = ScrollState::new(800.0);
        let t0 = Instant::now();
        scroll.on_scroll(100.0);

        // Simulate the renderer loop: tick returns true → retier.
        let settled = scroll.tick(t0 + SETTLE_DURATION + std::time::Duration::from_millis(10));
        assert!(settled);

        let mut cache = PageCache::new(u64::MAX);
        let result = cache.retier(&[page(0)], &scroll);
        // page 0 is not cached → should appear in rerender
        assert!(!result.rerender.is_empty());
    }
}
