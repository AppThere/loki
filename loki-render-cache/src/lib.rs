// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

#![forbid(unsafe_code)]

//! Page render primitives for Loki's Vello renderer.
//!
//! The [`PageSource`] trait (and [`RenderError`]) abstracts rendering a single
//! page to a GPU texture; GPU textures themselves are owned by `LokiPageSource`
//! instances inside Blitz's `CustomPaintSource` frame loop. The `gpu` feature
//! enables the wgpu-backed [`texture`] utilities used by the rendering layer in
//! `loki-renderer`.

pub mod key;
pub mod page_source;

#[cfg(feature = "gpu")]
pub mod texture;

pub use key::CacheKey;
pub use page_source::RenderError;

#[cfg(feature = "gpu")]
pub use page_source::PageSource;
#[cfg(feature = "gpu")]
pub use texture::GpuTexture;

/// Opaque index identifying a document page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageIndex(pub u32);
