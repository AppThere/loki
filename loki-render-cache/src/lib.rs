// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Tiered page render cache for Loki's Vello renderer.
//!
//! The cache stores only tier assignments and dirty flags — GPU textures are
//! owned by `LokiPageSource` instances inside Blitz's `CustomPaintSource`
//! frame loop.  The `gpu` feature enables wgpu texture utilities used by the
//! rendering layer in `loki-renderer`.

pub mod page_cache;
pub mod page_source;
pub mod retier;
pub mod scroll_state;
pub mod tier_policy;

#[cfg(feature = "gpu")]
pub mod texture;

pub use page_cache::{CachedPage, PageCache};
pub use page_source::RenderError;
pub use retier::RetierResult;
pub use scroll_state::{SETTLE_DURATION, ScrollPhase, ScrollState};
pub use tier_policy::{CacheTier, PageGeometry, assign_tier};

#[cfg(feature = "gpu")]
pub use page_source::PageSource;
#[cfg(feature = "gpu")]
pub use texture::{GpuTexture, allocate_texture, downsample_texture};

/// Opaque index identifying a document page within the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageIndex(pub u32);
