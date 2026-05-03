// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! LRU-tracked per-page tier-and-dirty metadata store.
//!
//! `CachedPage` no longer holds a GPU texture — GPU resources are owned by
//! `LokiPageSource` instances inside Blitz's `CustomPaintSource` frame loop.
//! The cache stores only the tier assignment and dirty flag needed by the
//! scroll-settle retier logic.

use std::collections::HashMap;

use crate::{CacheTier, PageIndex};

/// Metadata entry for a single cached page.
#[derive(Debug)]
pub struct CachedPage {
    /// The tier at which this page should currently be rendered.
    pub tier: CacheTier,
    /// `true` when document content has changed since the page was last
    /// rendered and a re-render is required.
    pub dirty: bool,
}

/// Tier-and-dirty metadata store for all document pages.
#[derive(Debug)]
pub struct PageCache {
    pub(crate) pages: HashMap<PageIndex, CachedPage>,
}

impl PageCache {
    /// Creates an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self { pages: HashMap::new() }
    }

    /// Inserts or replaces the tier entry for `index`, marking the page clean.
    pub fn insert(&mut self, index: PageIndex, tier: CacheTier) {
        self.pages.insert(index, CachedPage { tier, dirty: false });
    }

    /// Marks the page at `index` as dirty.
    ///
    /// Has no effect if the page is not in the cache.
    pub fn mark_dirty(&mut self, index: PageIndex) {
        if let Some(page) = self.pages.get_mut(&index) {
            page.dirty = true;
        }
    }

    /// Marks every cached page as dirty (e.g. after a document mutation).
    pub fn mark_all_dirty(&mut self) {
        for page in self.pages.values_mut() {
            page.dirty = true;
        }
    }

    /// Returns a shared reference to the cached page at `index`, or `None`
    /// if the page is not cached.
    #[must_use]
    pub fn get(&self, index: PageIndex) -> Option<&CachedPage> {
        self.pages.get(&index)
    }

    /// Returns `(hot, warm, cold)` page counts across all tiers.
    ///
    /// Useful for tracing and diagnostics.
    #[must_use]
    pub fn page_count_by_tier(&self) -> (usize, usize, usize) {
        let mut hot = 0usize;
        let mut warm = 0usize;
        let mut cold = 0usize;
        for page in self.pages.values() {
            match page.tier {
                CacheTier::Hot => hot += 1,
                CacheTier::Warm => warm += 1,
                CacheTier::Cold => cold += 1,
            }
        }
        (hot, warm, cold)
    }
}

impl Default for PageCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_then_get_returns_page_at_correct_tier() {
        let mut cache = PageCache::new();
        cache.insert(PageIndex(0), CacheTier::Hot);
        let page = cache.get(PageIndex(0)).expect("page should be cached");
        assert_eq!(page.tier, CacheTier::Hot);
        assert!(!page.dirty);
    }

    #[test]
    fn mark_dirty_sets_dirty_on_target_page_only() {
        let mut cache = PageCache::new();
        cache.insert(PageIndex(0), CacheTier::Hot);
        cache.insert(PageIndex(1), CacheTier::Hot);
        cache.mark_dirty(PageIndex(0));
        assert!(cache.get(PageIndex(0)).unwrap().dirty);
        assert!(!cache.get(PageIndex(1)).unwrap().dirty);
    }

    #[test]
    fn mark_all_dirty_sets_dirty_on_every_page() {
        let mut cache = PageCache::new();
        cache.insert(PageIndex(0), CacheTier::Hot);
        cache.insert(PageIndex(1), CacheTier::Warm);
        cache.insert(PageIndex(2), CacheTier::Cold);
        cache.mark_all_dirty();
        for idx in 0..3u32 {
            assert!(cache.get(PageIndex(idx)).unwrap().dirty, "page {idx} not dirty");
        }
    }

    #[test]
    fn get_on_missing_page_returns_none() {
        let cache = PageCache::new();
        assert!(cache.get(PageIndex(99)).is_none());
    }

    #[test]
    fn page_count_by_tier_returns_correct_counts() {
        let mut cache = PageCache::new();
        cache.insert(PageIndex(0), CacheTier::Hot);
        cache.insert(PageIndex(1), CacheTier::Hot);
        cache.insert(PageIndex(2), CacheTier::Warm);
        cache.insert(PageIndex(3), CacheTier::Cold);
        let (hot, warm, cold) = cache.page_count_by_tier();
        assert_eq!(hot, 2);
        assert_eq!(warm, 1);
        assert_eq!(cold, 1);
    }
}
