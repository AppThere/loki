// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! GPU texture allocation and downsampling blit.
//!
//! Only compiled when the `gpu` feature is active.

/// A GPU texture together with its pixel dimensions.
///
/// Wraps [`wgpu::Texture`] and records `width`/`height` so byte-budget
/// calculations (see [`crate::page_cache::PageCache::cold_bytes`]) don't
/// need a separate GPU query.
#[derive(Debug)]
pub struct GpuTexture {
    /// The underlying wgpu texture object.
    pub inner: wgpu::Texture,
    /// Width of the texture in pixels.
    pub width: u32,
    /// Height of the texture in pixels.
    pub height: u32,
}

impl GpuTexture {
    /// Returns the approximate GPU memory footprint in bytes (RGBA8 = 4 bytes
    /// per pixel, no mip-maps).
    #[must_use]
    pub fn byte_size(&self) -> u64 {
        self.width as u64 * self.height as u64 * 4
    }
}

/// Allocates a blank RGBA8 texture suitable for both rendering and sampling.
///
/// The texture has [`wgpu::TextureUsages::RENDER_ATTACHMENT`],
/// [`wgpu::TextureUsages::TEXTURE_BINDING`], and
/// [`wgpu::TextureUsages::COPY_SRC`].
#[must_use]
pub fn allocate_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    label: Option<&str>,
) -> GpuTexture {
    let inner = device.create_texture(&wgpu::TextureDescriptor {
        label,
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    GpuTexture {
        inner,
        width,
        height,
    }
}

/// Downsamples `src` into a new texture at `scale` × `src` dimensions.
///
/// Convenience wrapper that creates a temporary [`crate::blit::BlitPipeline`]
/// for a single call.  For repeated downsampling, prefer constructing a
/// `BlitPipeline` once and calling [`crate::blit::BlitPipeline::downsample`]
/// directly to avoid per-call pipeline compilation.
///
/// `scale` must be in `(0.0, 1.0]`; values > 1.0 are clamped to 1.0 so this
/// function never upsamples.
#[must_use]
pub fn downsample_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    src: &GpuTexture,
    scale: f32,
) -> GpuTexture {
    crate::blit::BlitPipeline::new(device).downsample(device, queue, src, scale)
}
