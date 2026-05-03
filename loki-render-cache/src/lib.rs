// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Tiered page render cache policy for Loki's Vello renderer.
//!
//! This crate provides the pure-logic layer that decides which cache tier each
//! document page belongs to based on scroll position. No GPU, no windowing —
//! just data structures and algorithms.
//!
//! # Overview
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │  Hot zone   │ 2 × viewport height centred on the visible area   │
//! │  Warm zone  │ Hot zone ± 3 × viewport height                    │
//! │  Cold zone  │ everything else                                    │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! When scrolling stops for [`scroll_state::SETTLE_DURATION`], the phase
//! transitions to [`scroll_state::ScrollPhase::Settling`] and the cache
//! should begin upgrading pages in the hot zone to full resolution via
//! [`page_cache::PageCache::retier`].

pub mod mock_texture;
pub mod page_cache;
pub mod retier;
pub mod scroll_state;
pub mod tier_policy;

pub use mock_texture::MockTexture;
pub use page_cache::{CachedPage, PageCache};
pub use retier::RetierResult;
pub use scroll_state::{SETTLE_DURATION, ScrollPhase, ScrollState};
pub use tier_policy::{CacheTier, PageGeometry, assign_tier};

/// Opaque index identifying a document page within the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageIndex(pub u32);

/// The texture type used throughout the cache.
///
/// Resolves to [`MockTexture`] when the `mock-texture` feature is active
/// (the default). Session 3 will introduce the real `wgpu::Texture` variant
/// behind a separate feature flag.
#[cfg(feature = "mock-texture")]
pub type PageTexture = MockTexture;
