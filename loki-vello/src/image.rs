// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Image rendering from data URIs.
//!
//! [`paint_image`] decodes a `data:image/...;base64,...` URI into raw RGBA
//! pixels, wraps them in a [`peniko::ImageData`], and draws the image into the
//! Vello scene using [`vello::Scene::draw_image`].
//!
//! External URLs are not fetched at runtime; instead a placeholder grey
//! rectangle is rendered in their place.

use std::sync::Arc;

use base64::Engine as _;
use loki_layout::{LayoutColor, PositionedImage, PositionedRect};

use crate::error::{VelloError, VelloResult};

/// Paint a positioned image into the scene.
///
/// Only `data:` URIs are decoded at runtime. Any other `src` value (e.g. an
/// `http://` URL) causes a grey placeholder rectangle to be rendered instead.
///
/// # Errors
///
/// Returns [`VelloError::ImageDecode`] if the data URI cannot be parsed or the
/// image bytes cannot be decoded.
pub fn paint_image(
    scene: &mut vello::Scene,
    item: &PositionedImage,
    scale: f32,
) -> VelloResult<()> {
    if !item.src.starts_with("data:") {
        // External URL: render a placeholder grey rectangle.
        let placeholder = PositionedRect {
            rect: item.rect,
            color: LayoutColor { r: 0.8, g: 0.8, b: 0.8, a: 1.0 },
        };
        crate::rect::paint_filled_rect(scene, &placeholder, scale);
        return Ok(());
    }

    let (rgba_bytes, width, height) = decode_data_uri(&item.src)?;

    // Wrap the raw pixel bytes in a Blob.
    let arc: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(rgba_bytes);
    let blob = peniko::Blob::new(arc);

    let image_data = peniko::ImageData {
        data: blob,
        format: peniko::ImageFormat::Rgba8,
        alpha_type: peniko::ImageAlphaType::Alpha,
        width,
        height,
    };

    let image_brush = peniko::ImageBrush::new(image_data);

    // Place the image at the layout rect's top-left corner.
    // draw_image renders the image at its natural pixel size starting at the
    // transformed origin.
    let transform = kurbo::Affine::translate((
        (item.rect.x() * scale) as f64,
        (item.rect.y() * scale) as f64,
    ));

    scene.draw_image(&image_brush, transform);
    Ok(())
}

/// Decode a `data:image/...;base64,...` URI into raw RGBA8 bytes and
/// dimensions.
///
/// # Supported formats
///
/// Any format supported by the `image` crate (PNG, JPEG, GIF, WebP, …).
///
/// # Errors
///
/// Returns [`VelloError::ImageDecode`] if the URI is malformed, the base64
/// payload is invalid, or the image bytes cannot be decoded.
fn decode_data_uri(src: &str) -> VelloResult<(Vec<u8>, u32, u32)> {
    // Find the comma that separates the metadata from the payload.
    let comma_pos = src.find(',').ok_or_else(|| VelloError::ImageDecode {
        reason: "no comma in data URI".into(),
    })?;
    let b64 = &src[comma_pos + 1..];

    let bytes =
        base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| VelloError::ImageDecode { reason: e.to_string() })?;

    let dyn_image = image::load_from_memory(&bytes)
        .map_err(|e| VelloError::ImageDecode { reason: e.to_string() })?;

    let rgba = dyn_image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok((rgba.into_raw(), width, height))
}
