// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! CPU twin of `loki_vello::image::paint_image`: decodes a `data:` URI into
//! RGBA8 pixels and fills the layout rect with them.
//!
//! Resolving [`PositionedImage::src`] is the renderer's job by design — the
//! field's own documentation in `loki-layout` says so — which is why this
//! mirrors the Vello implementation rather than sharing one, in the same way
//! `paint::paint_decoration` mirrors `loki_vello::decor::paint_decoration`.
//!
//! Until this existed the CPU path drew a flat grey rectangle for every image,
//! so every image-bearing region of a conformance render failed against a
//! golden that has the picture in it.

use loki_layout::{LayoutColor, PositionedImage, PositionedRect};
use vello_cpu::kurbo::{Affine, Rect};
use vello_cpu::{Image, ImageSource, RenderContext, peniko};

use crate::paint::paint_filled_rect;

/// Grey stand-in for an image this renderer cannot resolve.
const IMAGE_PLACEHOLDER: LayoutColor = LayoutColor {
    r: 0.8,
    g: 0.8,
    b: 0.8,
    a: 1.0,
};

/// Paint a positioned image, falling back to a grey rectangle when its `src`
/// cannot be resolved to pixels.
///
/// Only `data:` URIs are decoded: an external URL would mean a network fetch
/// from a rendering path that must stay deterministic and offline (Spec 02 D2),
/// so it keeps the placeholder.
///
/// The image is **scaled to the layout rect** rather than drawn at its natural
/// pixel size. The rect is the size the document asked for (`w:extent` for a
/// DOCX drawing); a 96 dpi asset placed in a 144 dpi render would otherwise
/// come out at two-thirds scale, and one authored oversized would overflow its
/// frame.
pub(crate) fn paint_image(
    ctx: &mut RenderContext,
    item: &PositionedImage,
    scale: f32,
    offset: (f32, f32),
) {
    let Some((rgba, w, h)) = decode_data_uri(&item.src) else {
        paint_filled_rect(
            ctx,
            &PositionedRect {
                rect: item.rect,
                color: IMAGE_PLACEHOLDER,
            },
            scale,
            offset,
        );
        return;
    };
    if w == 0 || h == 0 {
        return;
    }

    let data = peniko::ImageData {
        data: peniko::Blob::from(rgba),
        format: peniko::ImageFormat::Rgba8,
        alpha_type: peniko::ImageAlphaType::Alpha,
        width: w,
        height: h,
    };
    // `ImageBrush::new` exists only for the `ImageData` parameterisation;
    // vello_cpu's `Image` is `ImageBrush<ImageSource>`, so it is built
    // directly. `Pad` keeps edge pixels from wrapping when the destination
    // rounds a fraction of a pixel past the bitmap.
    let brush = Image {
        image: ImageSource::from_peniko_image_data(&data),
        sampler: peniko::ImageSampler::default().with_extend(peniko::Extend::Pad),
    };

    let dest = Rect::new(
        f64::from((item.rect.x() + offset.0) * scale),
        f64::from((item.rect.y() + offset.1) * scale),
        f64::from((item.rect.max_x() + offset.0) * scale),
        f64::from((item.rect.max_y() + offset.1) * scale),
    );
    // Maps the image's own pixel grid onto `dest`: the paint transform takes
    // paint space (0,0)-(w,h) into user space, so the fill samples the whole
    // bitmap across the destination rather than a `w`×`h` corner of it.
    let sx = dest.width() / f64::from(w);
    let sy = dest.height() / f64::from(h);
    ctx.set_paint(brush);
    ctx.set_paint_transform(
        Affine::translate((dest.x0, dest.y0)) * Affine::scale_non_uniform(sx, sy),
    );
    ctx.fill_rect(&dest);
    // The paint transform is sticky on the context; leave it as found so the
    // next solid fill is not silently drawn through this image's mapping.
    ctx.set_paint_transform(Affine::IDENTITY);
}

/// Decode a `data:image/…;base64,…` URI into RGBA8 bytes and dimensions.
///
/// `None` for anything that is not a decodable `data:` URI — a malformed URI,
/// an unsupported codec, or an external URL. The caller draws a placeholder;
/// a broken image must not abort a page render.
fn decode_data_uri(src: &str) -> Option<(Vec<u8>, u32, u32)> {
    use base64::Engine as _;

    let payload = src.strip_prefix("data:")?.split_once(',')?.1;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .ok()?;
    let rgba = ::image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some((rgba.into_raw(), w, h))
}

#[cfg(test)]
#[path = "image_tests.rs"]
mod tests;
