// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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

use quick_xml::events::Event;
use quick_xml::Reader;
use zip::CompressionMethod;
use zip::ZipArchive;

use crate::constants::{
    ENTRY_CONTENT, ENTRY_MANIFEST, ENTRY_META, ENTRY_MIMETYPE, ENTRY_SETTINGS,
    ENTRY_STYLES, MIME_ODT,
};
use crate::error::{OdfError, OdfResult};
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

    /// Raw bytes of `content.xml`. ODF 1.3 §3.1.
    pub content: Vec<u8>,

    /// Raw bytes of `styles.xml`. ODF 1.3 §3.1.
    pub styles: Vec<u8>,

    /// Raw bytes of `meta.xml`, or `None` if absent. ODF 1.3 §3.1.
    pub meta: Option<Vec<u8>>,

    /// Raw bytes of `settings.xml`, or `None` if absent. ODF 1.3 §3.1.
    pub settings: Option<Vec<u8>>,

    /// Images extracted from `Pictures/`: path → (media_type, bytes).
    ///
    /// The key is the full ZIP entry name (e.g. `"Pictures/image1.png"`).
    /// The media type is inferred from the file extension.
    pub images: HashMap<String, (String, Vec<u8>)>,

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
    pub fn open(reader: impl Read + Seek) -> OdfResult<Self> {
        let mut archive = ZipArchive::new(reader)?;

        // ── 1. Validate mimetype entry ─────────────────────────────────────
        validate_mimetype(&mut archive)?;

        // ── 2. Require META-INF/manifest.xml ──────────────────────────────
        {
            let _ = archive.by_name(ENTRY_MANIFEST).map_err(|_| {
                OdfError::MissingPart { part: ENTRY_MANIFEST.into() }
            })?;
        }

        // ── 3. Read content.xml (required) ────────────────────────────────
        let content = read_entry(&mut archive, ENTRY_CONTENT)?.ok_or_else(
            || OdfError::MissingPart { part: ENTRY_CONTENT.into() },
        )?;

        // ── 4. Read styles.xml (optional; fall back to empty element) ─────
        let styles = read_entry(&mut archive, ENTRY_STYLES)?.unwrap_or_else(
            || b"<office:document-styles/>".to_vec(),
        );

        // ── 5. Read optional parts ────────────────────────────────────────
        let meta = read_entry(&mut archive, ENTRY_META)?;
        let settings = read_entry(&mut archive, ENTRY_SETTINGS)?;

        // ── 6. Collect images from Pictures/ ─────────────────────────────
        let images = collect_images(&mut archive)?;

        // ── 7. Detect version from content.xml ────────────────────────────
        let (version, version_was_absent) = Self::detect_version(&content)?;

        Ok(Self {
            version,
            content,
            styles,
            meta,
            settings,
            images,
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
    pub fn detect_version(content: &[u8]) -> OdfResult<(OdfVersion, bool)> {
        let mut reader = Reader::from_reader(content);
        reader.config_mut().trim_text(false);

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let local = local_name_bytes(e.local_name().into_inner());
                    if local == b"document-content" || local == b"document" {
                        // Found the root element; look for office:version
                        let version_val = e
                            .attributes()
                            .flatten()
                            .find_map(|attr| {
                                let key =
                                    local_name_bytes(attr.key.as_ref());
                                if key == b"version" {
                                    attr.unescape_value()
                                        .ok()
                                        .map(|v| v.into_owned())
                                } else {
                                    None
                                }
                            });

                        return match version_val {
                            None => Ok((OdfVersion::V1_1, true)),
                            Some(s) => {
                                let v =
                                    OdfVersion::from_attr(&s).unwrap_or(
                                        OdfVersion::V1_3,
                                    );
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
) -> OdfResult<()> {
    if archive.len() == 0 {
        return Err(OdfError::MissingPart { part: ENTRY_MIMETYPE.into() });
    }

    let mut entry = archive.by_index(0)?;
    let name = entry.name().to_owned();

    if name != ENTRY_MIMETYPE {
        return Err(OdfError::MalformedElement {
            element: ENTRY_MIMETYPE.into(),
            part: ENTRY_MIMETYPE.into(),
            reason: format!(
                "first ZIP entry must be \"mimetype\", found \"{name}\""
            ),
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

    if buf != MIME_ODT.as_bytes() {
        return Err(OdfError::MalformedElement {
            element: ENTRY_MIMETYPE.into(),
            part: ENTRY_MIMETYPE.into(),
            reason: format!(
                "mimetype must contain {:?} with no trailing newline, \
                 found {:?}",
                MIME_ODT,
                String::from_utf8_lossy(&buf)
            ),
        });
    }

    Ok(())
}

/// Read a named ZIP entry into a `Vec<u8>`, returning `None` if absent.
fn read_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> OdfResult<Option<Vec<u8>>> {
    match archive.by_name(name) {
        Ok(mut entry) => {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            Ok(Some(buf))
        }
        Err(zip::result::ZipError::FileNotFound) => Ok(None),
        Err(e) => Err(OdfError::Zip(e)),
    }
}

/// Walk all ZIP entries, collect those under `Pictures/` with their inferred
/// media type.
fn collect_images<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> OdfResult<HashMap<String, (String, Vec<u8>)>> {
    let mut images = HashMap::new();

    // Collect names first to avoid borrow issues
    let names: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            archive.by_index(i).ok().map(|e| e.name().to_owned())
        })
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
fn infer_media_type(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else if lower.ends_with(".webp") {
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
mod tests {
    use std::io::{Cursor, Write};

    use zip::write::{FileOptions, ZipWriter};
    use zip::CompressionMethod;

    use super::*;
    use crate::version::OdfVersion;

    /// Build a minimal in-memory ODF ZIP with the given entries.
    ///
    /// `extra_entries` is a list of `(name, content, compressed)` tuples.
    fn build_zip(
        mimetype_first: bool,
        mimetype_content: &[u8],
        extra_entries: &[(&str, &[u8])],
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut zip = ZipWriter::new(Cursor::new(&mut buf));

        if mimetype_first {
            let opts = FileOptions::<()>::default()
                .compression_method(CompressionMethod::Stored);
            zip.start_file(ENTRY_MIMETYPE, opts).unwrap();
            zip.write_all(mimetype_content).unwrap();
        }

        for (name, data) in extra_entries {
            let opts = FileOptions::<()>::default()
                .compression_method(CompressionMethod::Deflated);
            zip.start_file(*name, opts).unwrap();
            zip.write_all(data).unwrap();
        }

        if !mimetype_first {
            let opts = FileOptions::<()>::default()
                .compression_method(CompressionMethod::Stored);
            zip.start_file(ENTRY_MIMETYPE, opts).unwrap();
            zip.write_all(mimetype_content).unwrap();
        }

        zip.finish().unwrap();
        buf
    }

    /// Minimal content.xml with a given version attribute (or absent).
    fn content_xml(version: Option<&str>) -> Vec<u8> {
        let ver_attr = match version {
            Some(v) => format!(" office:version=\"{v}\""),
            None => String::new(),
        };
        format!(
            r#"<?xml version="1.0"?><office:document-content{ver_attr}/>"#
        )
        .into_bytes()
    }

    fn minimal_zip(version: Option<&str>) -> Vec<u8> {
        let manifest = b"<manifest:manifest/>";
        let content = content_xml(version);
        build_zip(
            true,
            MIME_ODT.as_bytes(),
            &[
                (ENTRY_MANIFEST, manifest),
                (ENTRY_CONTENT, &content),
                (ENTRY_STYLES, b"<office:document-styles/>"),
            ],
        )
    }

    // ── open succeeds for well-formed package ─────────────────────────────

    #[test]
    fn open_minimal_package_succeeds() {
        let zip_bytes = minimal_zip(Some("1.3"));
        let result = OdfPackage::open(Cursor::new(zip_bytes));
        assert!(result.is_ok(), "Expected Ok, got {result:?}");
    }

    // ── mimetype must be first entry ──────────────────────────────────────

    #[test]
    fn open_mimetype_not_first_fails() {
        let content = content_xml(Some("1.2"));
        let zip_bytes = build_zip(
            false, // mimetype is NOT first
            MIME_ODT.as_bytes(),
            &[
                (ENTRY_MANIFEST, b"<manifest:manifest/>"),
                (ENTRY_CONTENT, &content),
            ],
        );
        let result = OdfPackage::open(Cursor::new(zip_bytes));
        assert!(
            matches!(result, Err(OdfError::MalformedElement { .. })),
            "Expected MalformedElement, got {result:?}"
        );
    }

    // ── mimetype with trailing newline fails ──────────────────────────────

    #[test]
    fn open_mimetype_trailing_newline_fails() {
        let mut mime_with_nl = MIME_ODT.as_bytes().to_vec();
        mime_with_nl.push(b'\n');

        let content = content_xml(Some("1.3"));
        let zip_bytes = build_zip(
            true,
            &mime_with_nl,
            &[
                (ENTRY_MANIFEST, b"<manifest:manifest/>"),
                (ENTRY_CONTENT, &content),
            ],
        );
        let result = OdfPackage::open(Cursor::new(zip_bytes));
        assert!(
            matches!(result, Err(OdfError::MalformedElement { .. })),
            "Expected MalformedElement, got {result:?}"
        );
    }

    // ── missing content.xml fails ─────────────────────────────────────────

    #[test]
    fn open_missing_content_xml_fails() {
        let zip_bytes = build_zip(
            true,
            MIME_ODT.as_bytes(),
            &[(ENTRY_MANIFEST, b"<manifest:manifest/>")],
        );
        let result = OdfPackage::open(Cursor::new(zip_bytes));
        assert!(
            matches!(result, Err(OdfError::MissingPart { ref part }) if part == ENTRY_CONTENT),
            "Expected MissingPart(content.xml), got {result:?}"
        );
    }

    // ── detect_version: office:version="1.2" ─────────────────────────────

    #[test]
    fn detect_version_1_2() {
        let content = content_xml(Some("1.2"));
        let (v, absent) = OdfPackage::detect_version(&content).unwrap();
        assert_eq!(v, OdfVersion::V1_2);
        assert!(!absent);
    }

    // ── detect_version: absent → V1_1, version_was_absent=true ───────────

    #[test]
    fn detect_version_absent_is_v1_1() {
        let content = content_xml(None);
        let (v, absent) = OdfPackage::detect_version(&content).unwrap();
        assert_eq!(v, OdfVersion::V1_1);
        assert!(absent);
    }

    // ── detect_version: unrecognised → V1_3, version_was_absent=false ────

    #[test]
    fn detect_version_unknown_falls_back_to_v1_3() {
        let content = content_xml(Some("99.0"));
        let (v, absent) = OdfPackage::detect_version(&content).unwrap();
        assert_eq!(v, OdfVersion::V1_3);
        assert!(!absent);
    }

    // ── infer_media_type covers standard extensions ───────────────────────

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
