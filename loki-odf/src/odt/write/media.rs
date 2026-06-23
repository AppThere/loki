// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Collects embedded image bytes for the ODT package's `Pictures/` subtree.
//!
//! Inline images carry their data as a `data:<media-type>;base64,…` URI (that is
//! how the importer round-trips them). On export each embedded image is decoded
//! and written as a `Pictures/imageN.<ext>` part, and the `draw:image` element
//! references that path — so image data is preserved across a save.

use base64::Engine as _;

/// One embedded media part destined for the package.
pub(crate) struct MediaPart {
    /// ZIP entry path, e.g. `"Pictures/image1.png"`.
    pub(crate) path: String,
    /// MIME media type, e.g. `"image/png"`.
    pub(crate) media_type: String,
    /// Raw image bytes.
    pub(crate) bytes: Vec<u8>,
}

/// One embedded object sub-document (e.g. a formula) destined for the package.
pub(crate) struct MathPart {
    /// Object directory path, e.g. `"Object 1"`.
    pub(crate) dir: String,
    /// The object's `content.xml` body (a `MathML` document).
    pub(crate) content_xml: String,
}

/// A rendered ODF part (XML) together with the image parts and embedded object
/// sub-documents it references.
pub(crate) struct Rendered {
    pub(crate) xml: String,
    pub(crate) media: Vec<MediaPart>,
    pub(crate) objects: Vec<MathPart>,
}

/// Accumulates the embedded images referenced by a document part.
///
/// The `prefix` distinguishes images from different parts (the body vs. the
/// master-page header/footer), so their `Pictures/` filenames never collide.
pub(super) struct Media {
    prefix: &'static str,
    parts: Vec<MediaPart>,
}

impl Media {
    /// Collector for the document body (`Pictures/image…`).
    pub(super) fn new() -> Self {
        Self::with_prefix("image")
    }

    /// Collector whose parts are named `Pictures/<prefix>…`.
    pub(super) fn with_prefix(prefix: &'static str) -> Self {
        Self {
            prefix,
            parts: Vec::new(),
        }
    }

    /// Resolves an image `url` to the `xlink:href` to use in the XML.
    ///
    /// A `data:` URI is decoded and stored as a new `Pictures/` part (returning
    /// that path); any other non-empty URL is treated as an external link and
    /// referenced as-is. Returns `None` for an empty or undecodable URL.
    pub(super) fn add_image(&mut self, url: &str) -> Option<String> {
        if let Some(rest) = url.strip_prefix("data:") {
            let (meta, data) = rest.split_once(',')?;
            if !meta.contains("base64") {
                return None;
            }
            let media_type = meta.split(';').next().unwrap_or("image/png").to_string();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data.trim())
                .ok()?;
            let path = format!(
                "Pictures/{}{}.{}",
                self.prefix,
                self.parts.len() + 1,
                ext_for(&media_type)
            );
            self.parts.push(MediaPart {
                path: path.clone(),
                media_type,
                bytes,
            });
            Some(path)
        } else if url.is_empty() {
            None
        } else {
            Some(url.to_string())
        }
    }

    /// Consumes the collector, returning the gathered image parts.
    pub(super) fn into_parts(self) -> Vec<MediaPart> {
        self.parts
    }
}

/// File extension for a raster/vector image MIME type.
fn ext_for(media_type: &str) -> &'static str {
    match media_type {
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "image/bmp" => "bmp",
        "image/tiff" => "tif",
        "image/webp" => "webp",
        _ => "png",
    }
}
