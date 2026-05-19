// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Tiered page-render cache — metadata only, no GPU resources.

pub mod retier;
pub mod tier;

use std::collections::HashMap;

use crate::key::CacheKey;
pub use tier::CacheTier;

/// Metadata for a single cached page.
#[derive(Debug)]
pub struct CachedPage {
    /// The tier at which this page should currently be rendered.
    pub tier: CacheTier,
    /// `true` when content has changed since the page was last rendered.
    pub dirty: bool,
}

/// Tier-and-dirty metadata store for all pages.
#[derive(Debug)]
pub struct PageCache<K: CacheKey> {
    pub(crate) pages: HashMap<K, CachedPage>,
}

impl<K: CacheKey> PageCache<K> {
    /// Creates an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pages: HashMap::new(),
        }
    }

    /// Inserts or replaces the tier entry for `index`, marking it clean.
    pub fn insert(&mut self, index: K, tier: CacheTier) {
        self.pages.insert(index, CachedPage { tier, dirty: false });
    }

    /// Marks the page at `index` as dirty (no-op if not cached).
    pub fn mark_dirty(&mut self, index: K) {
        if let Some(p) = self.pages.get_mut(&index) {
            p.dirty = true;
        }
    }

    /// Marks every cached page as dirty.
    pub fn mark_all_dirty(&mut self) {
        for p in self.pages.values_mut() {
            p.dirty = true;
        }
    }

    /// Returns the cached page at `index`, or `None`.
    #[must_use]
    pub fn get(&self, index: K) -> Option<&CachedPage> {
        self.pages.get(&index)
    }

    /// Returns `(hot, warm, cold)` page counts.
    #[must_use]
    pub fn page_count_by_tier(&self) -> (usize, usize, usize) {
        let mut hot = 0usize;
        let mut warm = 0usize;
        let mut cold = 0usize;
        for p in self.pages.values() {
            match p.tier {
                CacheTier::Hot => hot += 1,
                CacheTier::Warm => warm += 1,
                CacheTier::Cold => cold += 1,
            }
        }
        (hot, warm, cold)
    }
}

impl<K: CacheKey> Default for PageCache<K> {
    fn default() -> Self {
        Self::new()
    }
}
