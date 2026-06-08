// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tier reassignment logic.

use super::tier::{assign_tier, CacheTier, PageGeometry};
use super::PageCache;
use crate::key::CacheKey;
use crate::scroll::ScrollState;

/// Outcome of a single [`PageCache::retier`] call.
#[derive(Debug)]
pub struct RetierResult<K: CacheKey> {
    /// Pages whose tier has changed to finer resolution or are uncached.
    pub rerender: Vec<(K, CacheTier)>,
    /// Pages whose tier has been demoted (e.g. Hot→Warm).
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

fn quality(tier: CacheTier) -> u8 {
    match tier {
        CacheTier::Hot => 2,
        CacheTier::Warm => 1,
        CacheTier::Cold => 0,
    }
}

impl<K: CacheKey> PageCache<K> {
    /// Reassigns tiers for all `pages` based on `scroll`.
    pub fn retier(&mut self, pages: &[PageGeometry<K>], scroll: &ScrollState) -> RetierResult<K> {
        let mut result = RetierResult::default();
        for page_geom in pages {
            let idx = page_geom.index;
            let new_tier = assign_tier(page_geom, scroll);
            match self.pages.get_mut(&idx) {
                None => {
                    result.rerender.push((idx, new_tier));
                }
                Some(cached) => {
                    let old_q = quality(cached.tier);
                    let new_q = quality(new_tier);
                    if new_q > old_q {
                        cached.tier = new_tier;
                        cached.dirty = true;
                        result.rerender.push((idx, new_tier));
                    } else if new_q < old_q {
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
