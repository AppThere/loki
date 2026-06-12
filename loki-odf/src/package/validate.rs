// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ZIP entry reading and mimetype validation for ODF packages.

use std::io::{Read, Seek};

use zip::CompressionMethod;
use zip::ZipArchive;

use crate::constants::{ENTRY_MIMETYPE, MIME_ODS, MIME_ODT};
use crate::error::{OdfError, OdfResult};

/// Validate that the first ZIP entry is `mimetype`, uncompressed, containing
/// exactly [`MIME_ODT`] or [`MIME_ODS`] with no trailing newline.
///
/// ODF 1.3 §3.4.
pub(super) fn validate_mimetype<R: Read + Seek>(archive: &mut ZipArchive<R>) -> OdfResult<String> {
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

    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;

    let mimetype_str = String::from_utf8(buf).map_err(|_| OdfError::MalformedElement {
        element: ENTRY_MIMETYPE.into(),
        part: ENTRY_MIMETYPE.into(),
        reason: "mimetype entry contains invalid UTF-8".into(),
    })?;

    if mimetype_str != MIME_ODT && mimetype_str != MIME_ODS {
        return Err(OdfError::MalformedElement {
            element: ENTRY_MIMETYPE.into(),
            part: ENTRY_MIMETYPE.into(),
            reason: format!(
                "mimetype must contain either {MIME_ODT:?} or {MIME_ODS:?} with no trailing newline, \
                 found {mimetype_str:?}"
            ),
        });
    }

    Ok(mimetype_str)
}

/// Read a named ZIP entry into a `Vec<u8>`, returning `None` if absent.
pub(super) fn read_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> OdfResult<Option<Vec<u8>>> {
    match archive.by_name(name) {
        Ok(mut entry) => {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
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
pub(super) fn transcode_utf16_to_utf8(buf: &[u8]) -> Option<Vec<u8>> {
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
