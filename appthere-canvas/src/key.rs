// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`CacheKey`] marker trait for generic page key types.

/// Marker trait for types that can identify a page, used as
/// [`crate::PageSource::Key`].
///
/// Any type that satisfies `Hash + Eq + Copy + Send + Sync + 'static`
/// automatically implements `CacheKey` via the blanket impl below — no manual
/// implementation is required.
///
/// # Provided implementations
///
/// - [`crate::PageIndex`] — the default key for document pages.
/// - `u32`, `u64`, and any other primitive that satisfies the bounds.
pub trait CacheKey: std::hash::Hash + Eq + Copy + Send + Sync + 'static {}

impl<T: std::hash::Hash + Eq + Copy + Send + Sync + 'static> CacheKey for T {}
