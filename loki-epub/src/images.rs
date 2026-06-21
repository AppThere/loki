// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Image resource collection for EPUB packaging.
//!
//! Document images arrive as `data:` URIs (the OOXML/ODF importers base64-encode
//! embedded image parts). This module decodes those URIs into packaged
//! [`EpubImage`] resources and emits the `<img>` markup that references them.
//! External (non-`data:`) URLs are referenced as-is and not packaged.

use base64::Engine;

use crate::content::RenderCtx;
use crate::xml::{escape_attr, escape_text};

/// A decoded image to be written into the EPUB container and listed in the
/// package manifest.
pub struct EpubImage {
    /// Manifest item id (e.g. `img0`).
    pub id: String,
    /// Href relative to the `EPUB/` directory (e.g. `images/img0.png`).
    pub href: String,
    /// The EPUB core media type (e.g. `image/png`).
    pub media_type: String,
    /// The raw image bytes.
    pub bytes: Vec<u8>,
}

impl RenderCtx {
    /// Renders an inline image, packaging a `data:` URI as a resource or
    /// referencing an external URL directly.
    pub(crate) fn render_image(&mut self, url: &str, alt: &str, out: &mut String) {
        let alt_attr = escape_attr(alt);
        if let Some((media_type, bytes)) = decode_data_uri(url) {
            let ext = extension_for(&media_type);
            let id = format!("img{}", self.image_seq);
            let href = format!("images/{id}.{ext}");
            self.image_seq += 1;
            out.push_str(&format!(
                "<img src=\"{href}\" alt=\"{alt_attr}\"/>",
                href = escape_attr(&href),
            ));
            self.images.push(EpubImage {
                id,
                href,
                media_type,
                bytes,
            });
        } else if !url.is_empty() {
            // External URL — referenced but not packaged.
            out.push_str(&format!(
                "<img src=\"{src}\" alt=\"{alt_attr}\"/>",
                src = escape_attr(url),
            ));
        } else {
            // No usable source — fall back to the alt text so meaning survives.
            out.push_str(&escape_text(alt));
        }
    }
}

/// Decodes a base64 `data:image/...;base64,...` URI into `(media_type, bytes)`.
///
/// Returns `None` for non-`data:` URIs, non-image media types, or
/// non-base64 payloads (percent-encoded data URIs are rare for images).
pub(crate) fn decode_data_uri(uri: &str) -> Option<(String, Vec<u8>)> {
    let rest = uri.strip_prefix("data:")?;
    let comma = rest.find(',')?;
    let meta = &rest[..comma];
    let payload = &rest[comma + 1..];
    if !meta.contains(";base64") {
        return None;
    }
    let media_type = meta.split(';').next().unwrap_or("").to_ascii_lowercase();
    if !media_type.starts_with("image/") {
        return None;
    }
    let cleaned: String = payload.split_whitespace().collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(cleaned.as_bytes())
        .ok()?;
    Some((media_type, bytes))
}

/// Returns the filename extension for an EPUB core image media type.
fn extension_for(media_type: &str) -> &'static str {
    match media_type {
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "image/webp" => "webp",
        _ => "png",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_png_data_uri() {
        // "Hi" base64 = "SGk=".
        let (mt, bytes) = decode_data_uri("data:image/png;base64,SGk=").expect("decode");
        assert_eq!(mt, "image/png");
        assert_eq!(bytes, b"Hi");
    }

    #[test]
    fn rejects_external_url() {
        assert!(decode_data_uri("https://example.com/a.png").is_none());
    }

    #[test]
    fn rejects_non_image() {
        assert!(decode_data_uri("data:text/plain;base64,SGk=").is_none());
    }
}
