// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODF package (ZIP container) reading.
//!
//! [`OdfPackage`] opens a ZIP-based ODF archive and extracts the standard
//! parts (`content.xml`, `styles.xml`, `meta.xml`, `settings.xml`, and any
//! images) as raw byte vectors for subsequent parsing.
//!
//! This module does **not** parse the XML contents of any part. Validation of
//! element structure is left to the importers in [`crate::odt`].

use std::collections::HashMap;
use std::io::{Read, Seek};

use quick_xml::Reader;
use quick_xml::events::Event;
use zip::CompressionMethod;
use zip::ZipArchive;

use crate::constants::{
    ENTRY_CONTENT, ENTRY_MANIFEST, ENTRY_META, ENTRY_MIMETYPE, ENTRY_SETTINGS, ENTRY_STYLES,
    MIME_ODS, MIME_ODT, MIME_OTS, MIME_OTT,
};
use crate::error::{OdfError, OdfResult};
use crate::limits::read_entry_capped;
use crate::version::OdfVersion;

/// Contents of an opened ODF package.
///
/// Holds the raw bytes of each standard part so that callers can parse them
/// independently. Images are collected from the `Pictures/` subtree and stored
/// keyed by their ZIP entry path.
///
/// ODF 1.3 §3.3 (package structure), §3.4 (mimetype entry).
#[derive(Debug)]
pub struct OdfPackage {
    /// The detected ODF version of this package.
    pub version: OdfVersion,

    /// The detected ODF mimetype of this package.
    pub mimetype: String,

    /// Raw bytes of `content.xml`. ODF 1.3 §3.1.
    pub content: Vec<u8>,

    /// Raw bytes of `styles.xml`. ODF 1.3 §3.1.
    pub styles: Vec<u8>,

    /// Raw bytes of `meta.xml`, or `None` if absent. ODF 1.3 §3.1.
    pub meta: Option<Vec<u8>>,

    /// Raw bytes of `settings.xml`, or `None` if absent. ODF 1.3 §3.1.
    pub settings: Option<Vec<u8>>,

    /// Images extracted from `Pictures/`: path → (`media_type`, bytes).
    ///
    /// The key is the full ZIP entry name (e.g. `"Pictures/image1.png"`).
    /// The media type is inferred from the file extension.
    pub images: HashMap<String, (String, Vec<u8>)>,

    /// Embedded object sub-documents (e.g. formula objects): object directory
    /// → raw `content.xml` bytes. The key is the directory path without a
    /// trailing slash (e.g. `"Object 1"`), matching a `draw:object`'s
    /// `xlink:href` once `"./"` is stripped. ODF 1.3 §3.16.
    pub objects: HashMap<String, Vec<u8>>,

    /// `true` if the `office:version` attribute was absent in `content.xml`.
    ///
    /// An absent attribute is valid for ODF 1.1 documents; in that case the
    /// version is assumed to be [`OdfVersion::V1_1`].
    pub version_was_absent: bool,
}

impl OdfPackage {
    /// Open an ODF package from any `Read + Seek` source.
    ///
    /// Validates that:
    /// - the `mimetype` entry is first, uncompressed (`Stored`), and contains
    ///   exactly [`MIME_ODT`] with no trailing newline;
    /// - `META-INF/manifest.xml` is present;
    /// - `content.xml` is present.
    ///
    /// Does **not** validate the XML structure of any part.
    ///
    /// ODF 1.3 §3.3 (package structure), §3.4 (mimetype).
    ///
    /// # Errors
    ///
    /// Returns [`OdfError`] if the ZIP archive is invalid, the `mimetype` entry
    /// is missing or malformed, `META-INF/manifest.xml` is absent, or
    /// `content.xml` is absent.
    pub fn open(reader: impl Read + Seek) -> OdfResult<Self> {
        let mut archive = ZipArchive::new(reader)?;

        // Aggregate decompressed-byte budget for the whole package
        // (zip-bomb guard); threaded through every entry read.
        let mut total_decompressed: u64 = 0;

        // ── 1. Validate mimetype entry ─────────────────────────────────────
        let mimetype = validate_mimetype(&mut archive, &mut total_decompressed)?;

        // ── 2. Require META-INF/manifest.xml ──────────────────────────────
        {
            let _ = archive
                .by_name(ENTRY_MANIFEST)
                .map_err(|_| OdfError::MissingPart {
                    part: ENTRY_MANIFEST.into(),
                })?;
        }

        // ── 3. Read content.xml (required) ────────────────────────────────
        let content = read_entry(&mut archive, ENTRY_CONTENT, &mut total_decompressed)?
            .ok_or_else(|| OdfError::MissingPart {
                part: ENTRY_CONTENT.into(),
            })?;

        // ── 4. Read styles.xml (optional; fall back to empty element) ─────
        let styles = read_entry(&mut archive, ENTRY_STYLES, &mut total_decompressed)?
            .unwrap_or_else(|| b"<office:document-styles/>".to_vec());

        // ── 5. Read optional parts ────────────────────────────────────────
        let meta = read_entry(&mut archive, ENTRY_META, &mut total_decompressed)?;
        let settings = read_entry(&mut archive, ENTRY_SETTINGS, &mut total_decompressed)?;

        // ── 6. Collect images from Pictures/ ─────────────────────────────
        let images = collect_images(&mut archive, &mut total_decompressed)?;

        // ── 6b. Collect embedded object sub-documents (e.g. formulas) ─────
        let objects = collect_objects(&mut archive, &mut total_decompressed)?;

        // ── 7. Detect version from content.xml ────────────────────────────
        let (version, version_was_absent) = Self::detect_version(&content)?;

        Ok(Self {
            version,
            mimetype,
            content,
            styles,
            meta,
            settings,
            images,
            objects,
            version_was_absent,
        })
    }

    /// Detect the ODF version from the raw bytes of `content.xml`.
    ///
    /// Reads just enough of the XML to find the `office:version` attribute on
    /// the root element (`office:document-content` or `office:document`).
    ///
    /// - If the attribute is absent → `(V1_1, true)` (valid for ODF 1.1).
    /// - If the attribute is present and recognised → parsed version.
    /// - If the attribute is present but unrecognised → `(V1_3, false)`.
    ///   Callers that need to surface the warning should check
    ///   `version_was_absent == false` and compare against known versions.
    ///
    /// **Note**: an unrecognised version string cannot produce a warning here
    /// because this function returns only `OdfResult`. Callers should emit
    /// [`crate::error::OdfWarning::UnrecognisedVersion`] if appropriate.
    ///
    /// ODF 1.3 §3 (`office:version` attribute).
    ///
    /// # Errors
    ///
    /// Returns [`OdfError`] if the XML in `content` is malformed.
    pub fn detect_version(content: &[u8]) -> OdfResult<(OdfVersion, bool)> {
        let mut reader = Reader::from_reader(content);
        reader.config_mut().trim_text(false);

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                    let local = local_name_bytes(e.local_name().into_inner());
                    if local == b"document-content" || local == b"document" {
                        // Found the root element; look for office:version
                        let version_val = crate::xml_util::local_attr_val(e, b"version");

                        return match version_val {
                            None => Ok((OdfVersion::V1_1, true)),
                            Some(s) => {
                                let v = OdfVersion::from_attr(&s).unwrap_or(OdfVersion::V1_3);
                                Ok((v, false))
                            }
                        };
                    }
                    buf.clear();
                }
                Ok(Event::Eof) => {
                    // Root element not found; assume ODF 1.1
                    return Ok((OdfVersion::V1_1, true));
                }
                Err(e) => {
                    return Err(OdfError::Xml {
                        part: ENTRY_CONTENT.into(),
                        source: e,
                    });
                }
                _ => {
                    buf.clear();
                }
            }
        }
    }
}

// ── Private helpers ────────────────────────────────────────────────────────────

/// Validate that the first ZIP entry is `mimetype`, uncompressed, containing
/// exactly [`MIME_ODT`] with no trailing newline.
///
/// ODF 1.3 §3.4.
fn validate_mimetype<R: Read + Seek>(
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
fn read_entry<R: Read + Seek>(
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
fn collect_images<R: Read + Seek>(
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
fn collect_objects<R: Read + Seek>(
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

/// Extract the local name (bytes after last `:`) from a qualified name.
fn local_name_bytes(qname: &[u8]) -> &[u8] {
    if let Some(pos) = qname.iter().rposition(|&b| b == b':') {
        &qname[pos + 1..]
    } else {
        qname
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "package_tests.rs"]
mod tests;
