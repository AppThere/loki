// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! [`CacheKey`] marker trait for generic page-cache key types.

/// Marker trait for types that can serve as cache keys in [`crate::PageCache`].
///
/// Any type that satisfies `Hash + Eq + Copy + Send + Sync + 'static`
/// automatically implements `CacheKey` via the blanket impl below — no manual
/// implementation is required.
///
/// # Provided implementations
///
/// - [`crate::PageIndex`] — the default key for document-page caches.
/// - `u32`, `u64`, and any other primitive that satisfies the bounds.
pub trait CacheKey: std::hash::Hash + Eq + Copy + Send + Sync + 'static {}

impl<T: std::hash::Hash + Eq + Copy + Send + Sync + 'static> CacheKey for T {}
