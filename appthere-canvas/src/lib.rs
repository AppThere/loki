// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

#![forbid(unsafe_code)]

//! Generic GPU canvas infrastructure for AppThere applications.
//!
//! Feature flags:
//! - `gpu` — enables wgpu texture utilities and [`PageSource`].
//! - `dioxus` — enables Dioxus scroll helpers in [`crate::dioxus`].
//! - `font-cache` — enables [`FontDataCache`].

// The page-render cache, tier policy, scroll state, key trait, and GPU texture
// utilities are the canonical implementation in `loki-render-cache`, re-exported
// here so existing `appthere_canvas::*` paths keep working unchanged. This crate
// adds the Dioxus scroll driver and font cache on top.
pub use loki_render_cache::{
    assign_tier, CacheKey, CacheTier, CachedPage, PageCache, PageGeometry, PageIndex, RenderError,
    RetierResult, ScrollPhase, ScrollState, SETTLE_DURATION,
};

// Re-export the `texture` module too, so module-qualified paths such as
// `appthere_canvas::texture::GpuTexture` continue to resolve.
#[cfg(feature = "gpu")]
pub use loki_render_cache::texture;
#[cfg(feature = "gpu")]
pub use loki_render_cache::{
    allocate_texture, downsample_texture, BlitPipeline, GpuTexture, PageSource,
};

#[cfg(feature = "font-cache")]
pub mod font_cache;

#[cfg(feature = "dioxus")]
pub mod dioxus;

#[cfg(feature = "font-cache")]
pub use font_cache::FontDataCache;
