// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! [`CacheKey`] marker trait for generic cache key types.

/// Marker trait for types usable as keys in [`crate::cache::PageCache`].
///
/// Any type satisfying `Hash + Eq + Copy + Send + Sync + 'static` automatically
/// implements `CacheKey` via the blanket impl below — no manual implementation
/// required.
pub trait CacheKey: std::hash::Hash + Eq + Copy + Send + Sync + 'static {}

impl<T: std::hash::Hash + Eq + Copy + Send + Sync + 'static> CacheKey for T {}
