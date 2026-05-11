// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Image data extraction and media tracking.

use base64::Engine;

/// Extracts image bytes and file extension from a data URI.
/// Returns (extension, bytes) for supported types, None otherwise.
///
/// Supported: image/png → "png", image/jpeg → "jpg",
///            image/gif → "gif", image/webp → "webp"
pub fn extract_data_uri(src: &str) -> Option<(String, Vec<u8>)> {
    let src = src.strip_prefix("data:")?;
    let (mime_and_enc, b64) = src.split_once(',')?;
    let (mime, _enc) = mime_and_enc.split_once(';')?;
    let ext = match mime {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => return None,
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .ok()?;
    Some((ext.to_string(), bytes))
}
