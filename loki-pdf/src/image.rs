// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Image collection and embedding for the PDF exporter.
//!
//! Document images arrive as `data:` URIs. Each distinct image is decoded
//! once, converted to **DeviceCMYK** samples (so it matches the PDF/X CMYK
//! colour pipeline and the output intent), Flate-compressed, and embedded as an
//! image XObject. Transparency is preserved via a DeviceGray soft mask.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Write;

use base64::Engine;
use flate2::Compression;
use flate2::write::ZlibEncoder;
use pdf_writer::{Filter, Finish, Name, Pdf, Ref};

use crate::color::layout_to_cmyk;
use loki_layout::LayoutColor;

/// A decoded, CMYK, Flate-compressed image ready to embed.
pub struct ImageEntry {
    /// XObject resource name used in page resources (e.g. `Im0`).
    pub resource: String,
    width: i32,
    height: i32,
    cmyk_flate: Vec<u8>,
    alpha_flate: Option<Vec<u8>>,
}

/// Indirect references allocated for one image.
#[derive(Clone, Copy)]
pub struct ImageRefs {
    /// The image XObject (named in page resources).
    pub xobject: Ref,
    smask: Option<Ref>,
}

/// Collects distinct images used while content streams are built.
#[derive(Default)]
pub struct ImageBank {
    entries: Vec<ImageEntry>,
    // Maps a content hash to the entry index, or `None` for un-decodable images
    // so they are not retried per occurrence.
    by_hash: HashMap<u64, Option<usize>>,
}

impl ImageBank {
    /// Creates an empty bank.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers the image referenced by `src` (a `data:` URI), returning the
    /// XObject resource name to draw, or `None` when the image cannot be
    /// decoded or is an external reference.
    pub fn use_image(&mut self, src: &str) -> Option<String> {
        let mut hasher = DefaultHasher::new();
        src.hash(&mut hasher);
        let key = Hasher::finish(&hasher);

        let slot = match self.by_hash.get(&key) {
            Some(cached) => *cached,
            None => {
                let decoded = decode_and_convert(src, self.entries.len());
                let idx = decoded.map(|entry| {
                    self.entries.push(entry);
                    self.entries.len() - 1
                });
                self.by_hash.insert(key, idx);
                idx
            }
        };
        slot.map(|i| self.entries[i].resource.clone())
    }

    /// All registered images, in resource-name order.
    #[must_use]
    pub fn entries(&self) -> &[ImageEntry] {
        &self.entries
    }

    /// `true` when no image was registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Allocates the references each image needs (an XObject, plus a soft-mask
    /// XObject when the image has transparency), advancing `next`.
    pub fn allocate_refs(&self, next: &mut i32) -> Vec<ImageRefs> {
        self.entries
            .iter()
            .map(|entry| {
                let xobject = Ref::new(*next);
                *next += 1;
                let smask = entry.alpha_flate.as_ref().map(|_| {
                    let r = Ref::new(*next);
                    *next += 1;
                    r
                });
                ImageRefs { xobject, smask }
            })
            .collect()
    }

    /// Writes every image (and soft mask) XObject into `pdf`.
    pub fn embed(&self, pdf: &mut Pdf, refs: &[ImageRefs]) {
        for (entry, r) in self.entries.iter().zip(refs) {
            if let (Some(alpha), Some(smask_id)) = (&entry.alpha_flate, r.smask) {
                let mut mask = pdf.image_xobject(smask_id, alpha);
                mask.width(entry.width)
                    .height(entry.height)
                    .color_space_name(Name(b"DeviceGray"))
                    .bits_per_component(8)
                    .filter(Filter::FlateDecode);
                mask.finish();
            }

            let mut xobj = pdf.image_xobject(r.xobject, &entry.cmyk_flate);
            xobj.width(entry.width)
                .height(entry.height)
                .color_space_name(Name(b"DeviceCMYK"))
                .bits_per_component(8)
                .filter(Filter::FlateDecode);
            if let Some(smask_id) = r.smask {
                xobj.s_mask(smask_id);
            }
            xobj.finish();
        }
    }
}

/// Decodes a `data:` URI image and converts it to a CMYK [`ImageEntry`].
fn decode_and_convert(src: &str, index: usize) -> Option<ImageEntry> {
    let bytes = decode_data_uri(src)?;
    let img = image::load_from_memory(&bytes).ok()?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 {
        return None;
    }

    let mut cmyk = Vec::with_capacity((width * height * 4) as usize);
    let mut alpha = Vec::with_capacity((width * height) as usize);
    let mut has_alpha = false;
    for px in rgba.pixels() {
        let [r, g, b, a] = px.0;
        let c = layout_to_cmyk(LayoutColor::new(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
            1.0,
        ));
        cmyk.push(to_byte(c.c));
        cmyk.push(to_byte(c.m));
        cmyk.push(to_byte(c.y));
        cmyk.push(to_byte(c.k));
        alpha.push(a);
        if a != 255 {
            has_alpha = true;
        }
    }

    Some(ImageEntry {
        resource: format!("Im{index}"),
        width: width as i32,
        height: height as i32,
        cmyk_flate: deflate(&cmyk),
        alpha_flate: has_alpha.then(|| deflate(&alpha)),
    })
}

/// Decodes a base64 `data:image/...;base64,...` URI's payload to raw bytes.
fn decode_data_uri(uri: &str) -> Option<Vec<u8>> {
    let rest = uri.strip_prefix("data:")?;
    let comma = rest.find(',')?;
    let meta = &rest[..comma];
    if !meta.contains(";base64") || !meta.starts_with("image/") {
        return None;
    }
    let cleaned: String = rest[comma + 1..].split_whitespace().collect();
    base64::engine::general_purpose::STANDARD
        .decode(cleaned.as_bytes())
        .ok()
}

fn to_byte(component: f32) -> u8 {
    (component.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Zlib-compresses `data` for a PDF `FlateDecode` stream.
fn deflate(data: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    let _ = encoder.write_all(data);
    encoder.finish().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_url_is_skipped() {
        let mut bank = ImageBank::new();
        assert!(bank.use_image("https://example.com/a.png").is_none());
        assert!(bank.is_empty());
    }

    #[test]
    fn decodes_and_caches_png() {
        // 1x1 red PNG (RGBA) encoded once via the image crate.
        let mut buf = std::io::Cursor::new(Vec::new());
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, image::ImageFormat::Png)
            .unwrap();
        let uri = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(buf.get_ref())
        );

        let mut bank = ImageBank::new();
        let name = bank.use_image(&uri).expect("decode");
        assert_eq!(name, "Im0");
        // Second use of the same URI is cached (no new entry).
        assert_eq!(bank.use_image(&uri).as_deref(), Some("Im0"));
        assert_eq!(bank.entries().len(), 1);
    }
}
