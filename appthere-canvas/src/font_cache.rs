// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Cache for [`peniko::FontData`] instances keyed by pointer identity.
//!
//! Gate with the `font-cache` feature.

use std::collections::HashMap;
use std::sync::Arc;

/// Cache that maps raw font bytes (pointer address + index) to [`peniko::FontData`].
///
/// Construct one per rendering context and reuse across frames to avoid
/// redundant Blob allocations.
#[cfg(feature = "font-cache")]
#[derive(Default)]
pub struct FontDataCache {
    entries: HashMap<(usize, u32), peniko::FontData>,
}

#[cfg(feature = "font-cache")]
impl FontDataCache {
    /// Creates an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a [`peniko::FontData`] for the given font bytes and collection index.
    ///
    /// Two `Arc<Vec<u8>>` values pointing to the same allocation share a cache
    /// entry; different allocations with identical bytes are treated as distinct.
    pub fn get_or_insert(&mut self, data: &Arc<Vec<u8>>, font_index: u32) -> &peniko::FontData {
        let key = (Arc::as_ptr(data) as usize, font_index);
        self.entries.entry(key).or_insert_with(|| {
            let bytes: Vec<u8> = (**data).clone();
            let blob = peniko::Blob::from(bytes);
            peniko::FontData::new(blob, font_index)
        })
    }
}
