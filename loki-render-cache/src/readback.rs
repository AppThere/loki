// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! GPU → CPU texture readback and PNG data-URI encoding.
//!
//! [`texture_to_data_uri`] copies a [`GpuTexture`] to a CPU-mapped buffer,
//! strips wgpu row-alignment padding, encodes the raw RGBA8 pixels as a PNG
//! in memory, and returns a `data:image/png;base64,...` string ready for use
//! as an `img` element `src` attribute.
//!
//! This function **blocks the calling thread** via `device.poll(Wait)` and is
//! intended to be called only from the background render-worker thread, never
//! from the Dioxus/Blitz UI thread.

use std::sync::Arc;

use base64::Engine as _;
use thiserror::Error;

use crate::texture::GpuTexture;

/// Errors returned by [`texture_to_data_uri`].
#[derive(Debug, Error)]
pub enum ReadbackError {
    #[error("GPU buffer mapping failed: {0}")]
    Mapping(String),
    #[error("PNG encoding failed: {0}")]
    Png(String),
}

/// Copies `texture` to CPU memory and encodes it as a PNG data URI.
///
/// # Blocking
///
/// Calls `device.poll(Wait)` internally — blocks until the GPU copy is done.
/// Call only from a background worker thread.
pub fn texture_to_data_uri(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &GpuTexture,
) -> Result<Arc<String>, ReadbackError> {
    let width = texture.width;
    let height = texture.height;

    // wgpu requires rows to be aligned to COPY_BYTES_PER_ROW_ALIGNMENT bytes.
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let unpadded_bytes_per_row = width * 4;
    let padding = (align - unpadded_bytes_per_row % align) % align;
    let padded_bytes_per_row = unpadded_bytes_per_row + padding;
    let buffer_size = padded_bytes_per_row as u64 * height as u64;

    let readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("loki-readback"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("loki-readback-enc"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture.inner,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback_buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    queue.submit(std::iter::once(encoder.finish()));

    // Map and strip padding to get packed RGBA8 rows.
    let pixel_data = {
        let slice = readback_buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        device
            .poll(wgpu::PollType::Wait)
            .map_err(|e| ReadbackError::Mapping(e.to_string()))?;

        let mapped = slice.get_mapped_range();
        let mut data = Vec::with_capacity((unpadded_bytes_per_row * height) as usize);
        for row in 0..height {
            let start = (row * padded_bytes_per_row) as usize;
            let end = start + unpadded_bytes_per_row as usize;
            data.extend_from_slice(&mapped[start..end]);
        }
        data
    };
    readback_buf.unmap();

    // Encode as PNG in memory using the `image` crate.
    let img = image::RgbaImage::from_raw(width, height, pixel_data)
        .ok_or_else(|| ReadbackError::Png("pixel buffer size mismatch".into()))?;
    let mut png_buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut png_buf, image::ImageFormat::Png)
        .map_err(|e| ReadbackError::Png(e.to_string()))?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(png_buf.get_ref());
    Ok(Arc::new(format!("data:image/png;base64,{b64}")))
}
