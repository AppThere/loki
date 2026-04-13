// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Cache for [`peniko::FontData`] instances keyed by pointer identity.
//!
//! Font data bytes arrive from `loki-layout` as `Arc<Vec<u8>>`. Wrapping them
//! in a [`peniko::FontData`] (which internally creates a new [`peniko::Blob`]
//! with a unique ID) is cheap, but we avoid creating duplicate IDs for the same
//! allocation by caching based on the `Arc` pointer address combined with the
//! font collection index.

use std::collections::HashMap;
use std::sync::Arc;

/// Cache that maps raw font bytes (identified by pointer address + index) to a
/// [`peniko::FontData`].
///
/// Construct one per rendering context and reuse across frames to avoid
/// redundant Blob allocations.
#[derive(Default)]
pub struct FontDataCache {
    entries: HashMap<(usize, u32), peniko::FontData>,
}

impl FontDataCache {
    /// Creates an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a [`peniko::FontData`] for the given font bytes and collection index.
    ///
    /// If the same `(Arc` pointer, `font_index)` pair was seen before the cached
    /// value is returned, otherwise a new [`peniko::FontData`] is created and stored.
    ///
    /// # Key semantics
    ///
    /// Two `Arc<Vec<u8>>` values that point to the *same allocation* (i.e.
    /// clones of the same `Arc`) share a cache entry. Different allocations with
    /// identical byte content are treated as different fonts.
    pub fn get_or_insert(
        &mut self,
        data: &Arc<Vec<u8>>,
        font_index: u32,
    ) -> &peniko::FontData {
        let key = (Arc::as_ptr(data) as usize, font_index);
        self.entries.entry(key).or_insert_with(|| {
            // Clone only the Arc pointer (cheap), then convert to an owned Vec so
            // Blob::from can be used without needing an unsized coercion.
            // The clone is O(len) but happens at most once per unique font allocation.
            let bytes: Vec<u8> = (**data).clone();
            let blob = peniko::Blob::from(bytes);
            peniko::FontData::new(blob, font_index)
        })
    }
}
