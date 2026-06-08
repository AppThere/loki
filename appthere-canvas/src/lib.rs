// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

#![forbid(unsafe_code)]

//! Generic GPU canvas infrastructure for AppThere applications.
//!
//! Feature flags:
//! - `gpu` — enables wgpu texture utilities and [`PageSource`].
//! - `dioxus` — enables Dioxus scroll helpers in [`crate::dioxus`].
//! - `font-cache` — enables [`FontDataCache`].

pub mod cache;
pub mod key;
pub mod scroll;
pub mod source;

#[cfg(feature = "gpu")]
pub mod blit;
#[cfg(feature = "gpu")]
pub mod texture;

#[cfg(feature = "font-cache")]
pub mod font_cache;

#[cfg(feature = "dioxus")]
pub mod dioxus;

pub use cache::retier::RetierResult;
pub use cache::tier::{assign_tier, CacheTier, PageGeometry};
pub use cache::{CachedPage, PageCache};
pub use key::CacheKey;
pub use scroll::{ScrollPhase, ScrollState, SETTLE_DURATION};
pub use source::RenderError;

#[cfg(feature = "gpu")]
pub use blit::BlitPipeline;
#[cfg(feature = "gpu")]
pub use source::PageSource;
#[cfg(feature = "gpu")]
pub use texture::{allocate_texture, downsample_texture, GpuTexture};

#[cfg(feature = "font-cache")]
pub use font_cache::FontDataCache;

/// Opaque index identifying a document page within the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageIndex(pub u32);
