// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! GPU texture handle returned by [`crate::PageSource`].
//!
//! Only compiled when the `gpu` feature is active.

/// A GPU texture together with its pixel dimensions.
///
/// Wraps [`wgpu::Texture`] and records `width`/`height` so callers don't need a
/// separate GPU query to recover the texture's dimensions.
#[derive(Debug)]
pub struct GpuTexture {
    /// The underlying wgpu texture object.
    pub inner: wgpu::Texture,
    /// Width of the texture in pixels.
    pub width: u32,
    /// Height of the texture in pixels.
    pub height: u32,
}
