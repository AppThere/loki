// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Tiered page render cache for Loki's Vello renderer.
//!
//! # Features
//!
//! | Feature | Effect |
//! |---------|--------|
//! | `mock-texture` (default) | Uses [`MockTexture`] as [`PageTexture`]. Pure-std, no GPU deps. |
//! | `gpu` | Uses [`texture::GpuTexture`] backed by `wgpu`. Enables [`render_queue`] and [`page_source`]. |
//!
//! The two features are **mutually exclusive** — enabling both is a compile error.

#[cfg(all(feature = "mock-texture", feature = "gpu"))]
compile_error!("`mock-texture` and `gpu` features are mutually exclusive; \
                disable `default-features` when enabling `gpu`");

pub mod mock_texture;
pub mod page_cache;
pub mod page_source;
pub mod retier;
pub mod scroll_state;
pub mod tier_policy;

#[cfg(feature = "gpu")]
pub mod render_queue;
#[cfg(feature = "gpu")]
pub mod texture;

pub use mock_texture::MockTexture;
pub use page_cache::{CachedPage, PageCache};
pub use page_source::RenderError;
pub use retier::RetierResult;
pub use scroll_state::{SETTLE_DURATION, ScrollPhase, ScrollState};
pub use tier_policy::{CacheTier, PageGeometry, assign_tier};

#[cfg(feature = "gpu")]
pub use page_source::PageSource;
#[cfg(feature = "gpu")]
pub use render_queue::RenderQueue;
#[cfg(feature = "gpu")]
pub use texture::{GpuTexture, allocate_texture, downsample_texture};

/// Opaque index identifying a document page within the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageIndex(pub u32);

/// The texture type used throughout the cache.
///
/// Resolves to [`MockTexture`] under `mock-texture` (default), or
/// [`texture::GpuTexture`] under `gpu`.
#[cfg(feature = "mock-texture")]
pub type PageTexture = MockTexture;

#[cfg(feature = "gpu")]
pub type PageTexture = texture::GpuTexture;
