// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`PageSource`] trait and [`RenderError`] used by the render queue.

/// Errors returned by [`PageSource::render`].
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// The requested page index does not exist in the document.
    #[error("page {0} does not exist")]
    NoSuchPage(String),
    /// A wgpu-level error occurred during texture allocation or rendering.
    #[error("wgpu error: {0}")]
    Wgpu(String),
}

/// Supplies Vello-rendered page textures to the render queue.
///
/// Implementors must be `Send + Sync` because the render queue runs rendering
/// on a background thread.
///
/// The associated type [`PageSource::Key`] decouples the trait from any
/// particular key representation — use [`crate::PageIndex`] for document pages.
#[cfg(feature = "gpu")]
pub trait PageSource: Send + Sync {
    /// The cache key type used to identify pages.
    type Key: crate::key::CacheKey;

    /// Returns the logical page dimensions in pixels at 1.0× scale.
    fn page_size_px(&self, index: Self::Key) -> (u32, u32);

    /// Renders the page at `scale × page_size_px` and returns the rasterised
    /// texture. `scale` is the caller's device-pixel scale factor.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::NoSuchPage`] when `index` is out of range, or
    /// [`RenderError::Wgpu`] for GPU allocation/rendering failures.
    fn render(
        &self,
        index: Self::Key,
        scale: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<crate::texture::GpuTexture, RenderError>;
}
