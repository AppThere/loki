// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! LRU-tracked per-page texture store.

use std::collections::HashMap;
use std::sync::Arc;

use crate::{CacheTier, PageIndex, PageTexture};

/// A single page's texture entry in the cache.
#[derive(Debug)]
pub struct CachedPage {
    /// The tier at which this page was last rendered.
    pub tier: CacheTier,
    /// The rendered texture for this page.
    pub texture: PageTexture,
    /// `true` when document content has changed since the texture was
    /// rendered and a re-render is required.
    pub dirty: bool,
    /// LRU clock value at the time this page was last accessed or inserted.
    /// Higher values are more recently used.
    pub(crate) last_access: u64,
    /// PNG data URI (`data:image/png;base64,...`) for display in an `img`
    /// element.  Set asynchronously by the render worker after readback;
    /// `None` until the first readback completes or after eviction.
    pub data_uri: Option<Arc<String>>,
}

/// Texture store for all document pages, with an LRU clock and Cold-tier
/// byte-budget eviction.
#[derive(Debug)]
pub struct PageCache {
    pub(crate) pages: HashMap<PageIndex, CachedPage>,
    pub(crate) clock: u64,
    /// Maximum total bytes permitted in the Cold tier before the oldest Cold
    /// entries are evicted.
    pub cold_budget_bytes: u64,
}

impl PageCache {
    /// Creates an empty cache with the given Cold-tier byte budget.
    #[must_use]
    pub fn new(cold_budget_bytes: u64) -> Self {
        Self {
            pages: HashMap::new(),
            clock: 0,
            cold_budget_bytes,
        }
    }

    /// Inserts or replaces the rendered texture for `index`.
    ///
    /// Advances the LRU clock and marks the page clean.
    pub fn insert(&mut self, index: PageIndex, tier: CacheTier, texture: PageTexture) {
        self.clock += 1;
        self.pages.insert(
            index,
            CachedPage { tier, texture, dirty: false, last_access: self.clock, data_uri: None },
        );
    }

    /// Stores the PNG data URI for `index` after a GPU readback completes.
    ///
    /// Has no effect if the page is not in the cache (e.g. evicted between
    /// render completion and this call).
    pub fn set_data_uri(&mut self, index: PageIndex, uri: Arc<String>) {
        if let Some(page) = self.pages.get_mut(&index) {
            page.data_uri = Some(uri);
        }
    }

    /// Marks the page at `index` as dirty.
    ///
    /// Has no effect if the page is not in the cache.
    pub fn mark_dirty(&mut self, index: PageIndex) {
        if let Some(page) = self.pages.get_mut(&index) {
            page.dirty = true;
        }
    }

    /// Marks every cached page as dirty (e.g. after a zoom-level change).
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

    /// Returns the total texture bytes held across all tiers.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.pages.values().map(|p| p.texture.byte_size()).sum()
    }

    /// Returns the total texture bytes held in the Cold tier only.
    #[must_use]
    pub fn cold_bytes(&self) -> u64 {
        self.pages
            .values()
            .filter(|p| p.tier == CacheTier::Cold)
            .map(|p| p.texture.byte_size())
            .sum()
    }
}

#[cfg(all(test, feature = "mock-texture"))]
mod tests {
    use super::*;
    use crate::mock_texture::MockTexture;

    fn tex(w: u32, h: u32) -> MockTexture {
        MockTexture { width: w, height: h }
    }

    #[test]
    fn insert_then_get_returns_page_at_correct_tier() {
        let mut cache = PageCache::new(u64::MAX);
        cache.insert(PageIndex(0), CacheTier::Hot, tex(100, 100));
        let page = cache.get(PageIndex(0)).expect("page should be cached");
        assert_eq!(page.tier, CacheTier::Hot);
        assert!(!page.dirty);
    }

    #[test]
    fn total_bytes_and_cold_bytes_update_after_inserts() {
        let mut cache = PageCache::new(u64::MAX);
        cache.insert(PageIndex(0), CacheTier::Hot, tex(10, 10));   // 400 B
        cache.insert(PageIndex(1), CacheTier::Cold, tex(20, 20));  // 1600 B
        assert_eq!(cache.total_bytes(), 400 + 1600);
        assert_eq!(cache.cold_bytes(), 1600);
    }

    #[test]
    fn mark_dirty_sets_dirty_on_target_page_only() {
        let mut cache = PageCache::new(u64::MAX);
        cache.insert(PageIndex(0), CacheTier::Hot, tex(10, 10));
        cache.insert(PageIndex(1), CacheTier::Hot, tex(10, 10));
        cache.mark_dirty(PageIndex(0));
        assert!(cache.get(PageIndex(0)).unwrap().dirty);
        assert!(!cache.get(PageIndex(1)).unwrap().dirty);
    }

    #[test]
    fn mark_all_dirty_sets_dirty_on_every_page() {
        let mut cache = PageCache::new(u64::MAX);
        cache.insert(PageIndex(0), CacheTier::Hot, tex(10, 10));
        cache.insert(PageIndex(1), CacheTier::Warm, tex(10, 10));
        cache.insert(PageIndex(2), CacheTier::Cold, tex(10, 10));
        cache.mark_all_dirty();
        for idx in 0..3u32 {
            assert!(cache.get(PageIndex(idx)).unwrap().dirty, "page {idx} not dirty");
        }
    }

    #[test]
    fn get_on_missing_page_returns_none() {
        let cache = PageCache::new(u64::MAX);
        assert!(cache.get(PageIndex(99)).is_none());
    }
}
