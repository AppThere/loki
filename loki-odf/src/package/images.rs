// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Image collection from the `Pictures/` subtree of an ODF ZIP archive.

use std::collections::HashMap;
use std::io::{Read, Seek};

use zip::ZipArchive;

use crate::error::OdfResult;

/// Walk all ZIP entries, collect those under `Pictures/` with their inferred
/// media type.
pub(super) fn collect_images<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> OdfResult<HashMap<String, (String, Vec<u8>)>> {
    let mut images = HashMap::new();

    // Collect names first to avoid borrow issues
    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|e| e.name().to_owned()))
        .filter(|n| n.starts_with("Pictures/") && n.len() > "Pictures/".len())
        .collect();

    for name in names {
        if let Ok(mut entry) = archive.by_name(&name) {
            let media_type = infer_media_type(&name);
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            images.insert(name, (media_type.into(), bytes));
        }
    }

    Ok(images)
}

/// Infer a media type from a file extension (case-insensitive).
///
/// ODF 1.3 §3.16 (embedded objects / images).
pub(super) fn infer_media_type(path: &str) -> &'static str {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if ext.eq_ignore_ascii_case("png") {
        "image/png"
    } else if ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg") {
        "image/jpeg"
    } else if ext.eq_ignore_ascii_case("gif") {
        "image/gif"
    } else if ext.eq_ignore_ascii_case("svg") {
        "image/svg+xml"
    } else if ext.eq_ignore_ascii_case("webp") {
        "image/webp"
    } else {
        "application/octet-stream"
    }
}
