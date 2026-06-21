// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

#![forbid(unsafe_code)]

//! Generic GPU canvas infrastructure for AppThere applications.
//!
//! Feature flags:
//! - `gpu` — enables wgpu texture utilities and [`PageSource`].
//! - `font-cache` — enables [`FontDataCache`].

// The page-source trait, key trait, and GPU texture handle are the canonical
// implementation in `loki-render-cache`, re-exported here so existing
// `appthere_canvas::*` paths keep working unchanged. This crate adds the font
// cache on top.
pub use loki_render_cache::{CacheKey, PageIndex, RenderError};

// Re-export the `texture` module too, so module-qualified paths such as
// `appthere_canvas::texture::GpuTexture` continue to resolve.
#[cfg(feature = "gpu")]
pub use loki_render_cache::texture;
#[cfg(feature = "gpu")]
pub use loki_render_cache::{GpuTexture, PageSource};

#[cfg(feature = "font-cache")]
pub mod font_cache;

#[cfg(feature = "font-cache")]
pub use font_cache::FontDataCache;
