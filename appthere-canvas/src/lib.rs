// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

#![forbid(unsafe_code)]

//! Generic GPU canvas infrastructure for AppThere applications.
//!
//! Feature flags:
//! - `gpu` — enables wgpu texture utilities and [`PageSource`].
//! - `font-cache` — enables [`FontDataCache`].
//!
//! [`residency`] is unconditional: it is pure arithmetic plus a byte counter,
//! with no wgpu in it, so a headless bench can sweep the texture-memory model
//! without building the GPU stack (Spec 08 T2.5a).
//!
//! # Where the page-source types came from
//!
//! [`PageSource`], [`GpuTexture`], [`CacheKey`] and [`PageIndex`] used to live
//! in a separate `loki-render-cache` crate. That crate was named for a tiered
//! cache it did not contain — the Hot/Warm/Cold tiering was removed at some
//! point and nothing recorded it, so 115 lines of trait definitions kept a name
//! that made two Spec 08 r1 claims false. Spec 08 D-10 deleted it and moved the
//! types here, where the paths already resolved (this crate had re-exported
//! every one of them). ADR-0016 records the deletion and supersedes the
//! tiering decision.

pub mod key;
pub mod page_source;
pub mod residency;

#[cfg(feature = "gpu")]
pub mod texture;

pub use key::CacheKey;
pub use page_source::RenderError;
pub use residency::{TextureResidency, TextureResidencySnapshot};

#[cfg(feature = "gpu")]
pub use page_source::PageSource;
#[cfg(feature = "gpu")]
pub use texture::GpuTexture;

#[cfg(feature = "font-cache")]
pub mod font_cache;

#[cfg(feature = "font-cache")]
pub use font_cache::FontDataCache;

/// Opaque index identifying a document page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageIndex(pub u32);
