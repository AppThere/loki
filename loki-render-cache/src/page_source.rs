// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! [`PageSource`] trait and [`RenderError`] used by the render queue.

use crate::PageIndex;

/// Errors returned by [`PageSource::render`].
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// The requested page index does not exist in the document.
    #[error("page {0:?} does not exist")]
    NoSuchPage(PageIndex),
    /// A wgpu-level error occurred during texture allocation or rendering.
    #[error("wgpu error: {0}")]
    Wgpu(String),
}

/// Supplies Vello-rendered page textures to the render queue.
///
/// Implemented by the Loki document layer (`loki-text`) so that
/// `loki-render-cache` never depends on `loki-doc-model` directly.
///
/// Implementors must be `Send + Sync` because [`crate::render_queue::RenderQueue`]
/// runs rendering on a background thread.
#[cfg(feature = "gpu")]
pub trait PageSource: Send + Sync {
    /// Returns the logical page dimensions in pixels at 1.0× scale.
    fn page_size_px(&self, index: PageIndex) -> (u32, u32);

    /// Renders the page at `scale × page_size_px` and returns the rasterised
    /// texture. `scale` comes from [`crate::tier_policy::CacheTier::scale_factor`].
    ///
    /// # Errors
    ///
    /// Returns [`RenderError::NoSuchPage`] when `index` is out of range, or
    /// [`RenderError::Wgpu`] for GPU allocation/rendering failures.
    fn render(
        &self,
        index: PageIndex,
        scale: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<crate::texture::GpuTexture, RenderError>;
}
