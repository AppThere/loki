// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ZIP-entry reading helpers for [`super::OdfPackage`], split out of
//! `package.rs` for the 300-line ceiling: mimetype validation, capped entry
//! reads with UTF-16→UTF-8 transcoding, and the `Pictures/` image and embedded
//! `content.xml` object collectors. The parent re-imports the `pub(super)`
//! entry points; `transcode_utf16_to_utf8` and `infer_media_type` are internal.

use std::collections::HashMap;
use std::io::{Read, Seek};

use zip::CompressionMethod;
use zip::ZipArchive;

use crate::constants::{ENTRY_MIMETYPE, MIME_ODS, MIME_ODT, MIME_OTS, MIME_OTT};
use crate::error::{OdfError, OdfResult};
use crate::limits::read_entry_capped;

/// Validate that the first ZIP entry is `mimetype`, uncompressed, containing
/// exactly [`MIME_ODT`] with no trailing newline.
///
/// ODF 1.3 §3.4.
pub(super) fn validate_mimetype<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    total_decompressed: &mut u64,
) -> OdfResult<String> {
    if archive.is_empty() {
        return Err(OdfError::MissingPart {
            part: ENTRY_MIMETYPE.into(),
        });
    }

    let mut entry = archive.by_index(0)?;
    let name = entry.name().to_owned();

    if name != ENTRY_MIMETYPE {
        return Err(OdfError::MalformedElement {
            element: ENTRY_MIMETYPE.into(),
            part: ENTRY_MIMETYPE.into(),
            reason: format!("first ZIP entry must be \"mimetype\", found \"{name}\""),
        });
    }

    if entry.compression() != CompressionMethod::Stored {
        return Err(OdfError::MalformedElement {
            element: ENTRY_MIMETYPE.into(),
            part: ENTRY_MIMETYPE.into(),
            reason: "mimetype entry must be stored (uncompressed)".into(),
        });
    }

    let buf = read_entry_capped(&mut entry, ENTRY_MIMETYPE, total_decompressed)?;

    let mimetype_str = String::from_utf8(buf).map_err(|_| OdfError::MalformedElement {
        element: ENTRY_MIMETYPE.into(),
        part: ENTRY_MIMETYPE.into(),
        reason: "mimetype entry contains invalid UTF-8".into(),
    })?;

    // Accept document packages (ODT/ODS) and their template variants
    // (OTT/OTS). A template is structurally identical to its document form; the
    // editor opens it as a new untitled document.
    if !matches!(
        mimetype_str.as_str(),
        MIME_ODT | MIME_ODS | MIME_OTT | MIME_OTS
    ) {
        return Err(OdfError::MalformedElement {
            element: ENTRY_MIMETYPE.into(),
            part: ENTRY_MIMETYPE.into(),
            reason: format!(
                "mimetype must contain one of {MIME_ODT:?}, {MIME_ODS:?}, {MIME_OTT:?}, or \
                 {MIME_OTS:?} with no trailing newline, found {mimetype_str:?}"
            ),
        });
    }

    Ok(mimetype_str)
}

/// Read a named ZIP entry into a `Vec<u8>`, returning `None` if absent.
pub(super) fn read_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
    total_decompressed: &mut u64,
) -> OdfResult<Option<Vec<u8>>> {
    match archive.by_name(name) {
        Ok(mut entry) => {
            let buf = read_entry_capped(&mut entry, name, total_decompressed)?;
            if let Some(transcoded) = transcode_utf16_to_utf8(&buf) {
                Ok(Some(transcoded))
            } else {
                Ok(Some(buf))
            }
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(e) => Err(OdfError::Zip(e)),
    }
}

/// Transcode a UTF-16 (BE or LE) XML buffer to UTF-8 on the fly.
fn transcode_utf16_to_utf8(buf: &[u8]) -> Option<Vec<u8>> {
    if buf.len() < 2 {
        return None;
    }
    let big_endian = match (buf[0], buf[1]) {
        (0xFE, 0xFF) => true,
        (0xFF, 0xFE) => false,
        _ => return None,
    };

    let u16_data: Vec<u16> = if big_endian {
        buf[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect()
    } else {
        buf[2..]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect()
    };

    let string = String::from_utf16_lossy(&u16_data);
    Some(string.into_bytes())
}

/// Walk all ZIP entries, collect those under `Pictures/` with their inferred
/// media type.
pub(super) fn collect_images<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    total_decompressed: &mut u64,
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
            let bytes = read_entry_capped(&mut entry, &name, total_decompressed)?;
            images.insert(name, (media_type.into(), bytes));
        }
    }

    Ok(images)
}

/// Walk all ZIP entries, collecting embedded object sub-documents — any
/// `<dir>/content.xml` other than the package root `content.xml`. The key is
/// the directory path (no trailing slash). ODF 1.3 §3.16.
pub(super) fn collect_objects<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    total_decompressed: &mut u64,
) -> OdfResult<HashMap<String, Vec<u8>>> {
    let mut objects = HashMap::new();

    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|e| e.name().to_owned()))
        .filter(|n| n.ends_with("/content.xml"))
        .collect();

    for name in names {
        let Some(dir) = name.strip_suffix("/content.xml") else {
            continue;
        };
        if dir.is_empty() {
            continue;
        }
        if let Ok(mut entry) = archive.by_name(&name) {
            let bytes = read_entry_capped(&mut entry, &name, total_decompressed)?;
            objects.insert(dir.to_string(), bytes);
        }
    }

    Ok(objects)
}

/// Infer a media type from a file extension (case-insensitive).
///
/// ODF 1.3 §3.16 (embedded objects / images).
fn infer_media_type(path: &str) -> &'static str {
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

#[cfg(test)]
mod tests {
    use super::infer_media_type;

    #[test]
    fn infer_media_type_png() {
        assert_eq!(infer_media_type("Pictures/img.png"), "image/png");
    }

    #[test]
    fn infer_media_type_jpeg() {
        assert_eq!(infer_media_type("Pictures/photo.jpg"), "image/jpeg");
        assert_eq!(infer_media_type("Pictures/photo.jpeg"), "image/jpeg");
    }

    #[test]
    fn infer_media_type_svg() {
        assert_eq!(infer_media_type("Pictures/logo.svg"), "image/svg+xml");
    }

    #[test]
    fn infer_media_type_unknown() {
        assert_eq!(
            infer_media_type("Pictures/file.tiff"),
            "application/octet-stream"
        );
    }
}
