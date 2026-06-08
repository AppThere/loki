// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`PageSource`] trait and [`RenderError`] for the render queue.

/// Errors returned by [`PageSource::render`].
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// The requested page does not exist in the document.
    #[error("page {0} does not exist")]
    NoSuchPage(String),
    /// A wgpu-level error occurred during texture allocation or rendering.
    #[error("wgpu error: {0}")]
    Wgpu(String),
}

/// Supplies Vello-rendered page textures to the render queue.
///
/// The associated type [`PageSource::Key`] decouples the trait from any
/// particular key representation — use a `PageIndex` newtype for document pages.
#[cfg(feature = "gpu")]
pub trait PageSource: Send + Sync {
    /// Cache key type used to identify pages.
    type Key: crate::key::CacheKey;

    /// Returns the logical page dimensions in pixels at 1.0× scale.
    fn page_size_px(&self, index: Self::Key) -> (u32, u32);

    /// Renders the page at `scale × page_size_px` and returns the rasterised texture.
    fn render(
        &self,
        index: Self::Key,
        scale: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<crate::texture::GpuTexture, RenderError>;
}
