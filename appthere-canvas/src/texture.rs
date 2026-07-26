// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! GPU texture handle returned by [`crate::PageSource`].
//!
//! Only compiled when the `gpu` feature is active.

use crate::residency::{TextureResidency, texture_bytes};

/// A GPU texture together with its pixel dimensions.
///
/// Wraps [`wgpu::Texture`] and records `width`/`height` so callers don't need a
/// separate GPU query to recover the texture's dimensions.
///
/// # Residency accounting is structural (Spec 08 T2.5)
///
/// Construction records the allocation with [`TextureResidency`] and `Drop`
/// records the release, so a texture obtained this way cannot escape the count.
/// The fields are private for exactly that reason: a public `GpuTexture { .. }`
/// literal would allocate without recording, and the resulting drift is
/// invisible — residency simply reads low forever after. One constructor, one
/// release path, enforced by the type rather than by reviewers remembering.
///
/// The other allocation site — `loki_renderer::page_paint_render` — hands its
/// texture straight to Blitz and never wraps it here, so it records by hand.
/// That asymmetry is the reason this type does not carry the accounting for the
/// whole workspace, and it is why the release sites there are gathered into one
/// file.
#[derive(Debug)]
pub struct GpuTexture {
    /// The underlying wgpu texture object.
    inner: wgpu::Texture,
    /// Width of the texture in pixels.
    width: u32,
    /// Height of the texture in pixels.
    height: u32,
}

impl GpuTexture {
    /// Takes ownership of a freshly allocated `wgpu::Texture` of
    /// `width` × `height`, recording the allocation.
    ///
    /// The dimensions are the caller's — the same numbers passed to
    /// `TextureDescriptor` — because the residency model is keyed on what was
    /// requested.
    #[must_use]
    pub fn new(inner: wgpu::Texture, width: u32, height: u32) -> Self {
        TextureResidency::record_alloc(texture_bytes(width, height));
        Self {
            inner,
            width,
            height,
        }
    }

    /// The underlying wgpu texture.
    #[must_use]
    pub fn texture(&self) -> &wgpu::Texture {
        &self.inner
    }

    /// Width in pixels.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }
}

impl Drop for GpuTexture {
    fn drop(&mut self) {
        TextureResidency::record_free(texture_bytes(self.width, self.height));
    }
}
