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

mod images;
mod validate;
mod xml_util;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::io::{Read, Seek};

use quick_xml::Reader;
use quick_xml::events::Event;
use zip::ZipArchive;

use crate::constants::{ENTRY_CONTENT, ENTRY_MANIFEST, ENTRY_META, ENTRY_SETTINGS, ENTRY_STYLES};
use crate::error::{OdfError, OdfResult};
use crate::version::OdfVersion;

use images::collect_images;
use validate::{read_entry, validate_mimetype};
use xml_util::local_name_bytes;

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

        // ── 1. Validate mimetype entry ─────────────────────────────────────
        let mimetype = validate_mimetype(&mut archive)?;

        // ── 2. Require META-INF/manifest.xml ──────────────────────────────
        {
            let _ = archive
                .by_name(ENTRY_MANIFEST)
                .map_err(|_| OdfError::MissingPart {
                    part: ENTRY_MANIFEST.into(),
                })?;
        }

        // ── 3. Read content.xml (required) ────────────────────────────────
        let content =
            read_entry(&mut archive, ENTRY_CONTENT)?.ok_or_else(|| OdfError::MissingPart {
                part: ENTRY_CONTENT.into(),
            })?;

        // ── 4. Read styles.xml (optional; fall back to empty element) ─────
        let styles = read_entry(&mut archive, ENTRY_STYLES)?
            .unwrap_or_else(|| b"<office:document-styles/>".to_vec());

        // ── 5. Read optional parts ────────────────────────────────────────
        let meta = read_entry(&mut archive, ENTRY_META)?;
        let settings = read_entry(&mut archive, ENTRY_SETTINGS)?;

        // ── 6. Collect images from Pictures/ ─────────────────────────────
        let images = collect_images(&mut archive)?;

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
                        let version_val = e.attributes().flatten().find_map(|attr| {
                            let key = local_name_bytes(attr.key.as_ref());
                            if key == b"version" {
                                attr.unescape_value().ok().map(std::borrow::Cow::into_owned)
                            } else {
                                None
                            }
                        });

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
