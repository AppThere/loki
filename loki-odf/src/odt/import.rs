// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! ODT import entry point.
//!
//! [`OdtImport`] implements [`loki_doc_model::io::DocumentImport`] and is the
//! primary public API for converting an ODT file into a
//! [`loki_doc_model::Document`].
//!
//! The current implementation opens and validates the ODF package and records
//! the source version; document content parsing will be added in later
//! sessions.
//!
//! # Round-trip version rule
//!
//! The detected [`OdfVersion`] is stored in
//! [`OdtImportResult::source_version`] and written into the document's
//! [`loki_doc_model::io::DocumentSource`]. Exporters read this field so that
//! a document round-tripped through this crate is emitted at the same ODF
//! version as its source.

use std::io::{Read, Seek};

use loki_doc_model::document::Document;
use loki_doc_model::io::source::DocumentSource;
use loki_doc_model::io::DocumentImport;

use crate::error::{OdfError, OdfResult, OdfWarning};
use crate::package::OdfPackage;
use crate::version::OdfVersion;

// ── Options ────────────────────────────────────────────────────────────────────

/// Options controlling ODT import behaviour.
///
/// ODF 1.3 §3 (package conventions).
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct OdtImportOptions {
    /// When `true`, paragraphs whose style name starts with `"Heading"` are
    /// mapped to [`loki_doc_model::content::block::Block::Heading`] rather
    /// than plain paragraph blocks.
    ///
    /// Defaults to `true`.
    pub emit_heading_blocks: bool,

    /// When `true`, images are embedded in the document as data URIs
    /// (`data:<media-type>;base64,<data>`). When `false`, images are omitted.
    ///
    /// Defaults to `true`.
    pub embed_images: bool,

    /// When `true`, an unrecognised `office:version` attribute causes
    /// [`OdfError::UnsupportedVersion`] to be returned. When `false`
    /// (default), the version is treated as the latest supported version and
    /// an [`OdfWarning::UnrecognisedVersion`] is emitted instead.
    ///
    /// ODF 1.3 §3 (`office:version` attribute).
    pub strict_version: bool,
}

impl Default for OdtImportOptions {
    fn default() -> Self {
        Self {
            emit_heading_blocks: true,
            embed_images: true,
            strict_version: false,
        }
    }
}

// ── Result ─────────────────────────────────────────────────────────────────────

/// The result of a successful ODT import.
///
/// ODF 1.3 §3 (package conventions).
#[derive(Debug)]
pub struct OdtImportResult {
    /// The imported document in the format-neutral abstract model.
    pub document: Document,

    /// Non-fatal issues encountered during import.
    pub warnings: Vec<OdfWarning>,

    /// The ODF version detected in the source file.
    ///
    /// Exporters should use this value to write the document back at the same
    /// version, preserving the round-trip contract.
    pub source_version: OdfVersion,
}

// ── Public API ─────────────────────────────────────────────────────────────────

/// Unit struct that implements [`DocumentImport`] for ODT files.
///
/// Warnings are discarded. Use [`OdtImporter`] to retrieve them.
///
/// ODF 1.3 §3 (package conventions).
pub struct OdtImport;

impl DocumentImport for OdtImport {
    type Error = OdfError;
    type Options = OdtImportOptions;

    /// Import an ODT file and return the abstract document.
    ///
    /// Warnings are discarded. Use [`OdtImporter`] to retrieve them.
    ///
    /// ODF 1.3 §3 (package conventions).
    fn import(
        reader: impl Read + Seek,
        options: Self::Options,
    ) -> OdfResult<Document> {
        OdtImporter::new(options).run(reader).map(|r| r.document)
    }
}

/// Stateful ODT importer that preserves [`OdfWarning`]s alongside the
/// imported [`Document`].
///
/// Use this type when you need to inspect non-fatal import issues or access
/// [`OdtImportResult::source_version`].
///
/// ODF 1.3 §3 (package conventions).
pub struct OdtImporter {
    options: OdtImportOptions,
}

impl OdtImporter {
    /// Creates a new importer with the given options.
    #[must_use]
    pub fn new(options: OdtImportOptions) -> Self {
        Self { options }
    }

    /// Opens the ODT container, validates the package structure, parses all
    /// XML parts, and returns an [`OdtImportResult`] with a fully populated
    /// [`Document`].
    ///
    /// # Errors
    ///
    /// - [`OdfError::Zip`] — the archive is malformed.
    /// - [`OdfError::MalformedElement`] — the `mimetype` entry is invalid.
    /// - [`OdfError::MissingPart`] — `content.xml` or
    ///   `META-INF/manifest.xml` is absent.
    /// - [`OdfError::UnsupportedVersion`] — `strict_version` is `true` and
    ///   the version attribute holds an unrecognised value.
    /// - [`OdfError::Xml`] — any part contains malformed XML.
    ///
    /// ODF 1.3 §3 (package conventions).
    pub fn run(
        self,
        reader: impl Read + Seek,
    ) -> OdfResult<OdtImportResult> {
        let mut warnings = Vec::new();

        let package = OdfPackage::open(reader)?;

        // ── Version detection ─────────────────────────────────────────────
        // If the version was absent and strict mode is off, that is valid
        // (ODF 1.1). If it was present but unrecognised and strict mode is
        // on, raise an error.
        let source_version = if !package.version_was_absent
            && package.version == OdfVersion::V1_3
        {
            // detect_version returns V1_3 as a fallback for unknown strings;
            // re-examine the raw attribute to surface an
            // UnrecognisedVersion warning when appropriate.
            let raw = raw_version_attr(&package.content);
            match raw {
                Some(ref s) if OdfVersion::from_attr(s).is_none() => {
                    if self.options.strict_version {
                        return Err(OdfError::UnsupportedVersion {
                            version: s.clone(),
                        });
                    }
                    warnings.push(OdfWarning::UnrecognisedVersion {
                        version: s.clone(),
                    });
                    OdfVersion::V1_3
                }
                _ => package.version,
            }
        } else {
            package.version
        };

        // ── Parse XML parts ───────────────────────────────────────────────
        let odf_doc =
            crate::odt::reader::document::read_document(&package.content)?;

        let mut stylesheet =
            crate::odt::reader::styles::read_stylesheet(&package.styles, false)?;

        // Merge automatic styles from content.xml (paragraph/span-level styles
        // that are specific to this document instance).
        let auto_styles =
            crate::odt::reader::styles::read_auto_styles(&package.content)?;
        stylesheet.merge_auto(auto_styles);

        let odf_meta = package
            .meta
            .as_deref()
            .map(crate::odt::reader::meta::read_meta)
            .transpose()?;

        // ── Map to document model ─────────────────────────────────────────
        let (mut document, mut mapper_warnings) =
            crate::odt::mapper::document::map_document(
                &odf_doc,
                &stylesheet,
                odf_meta.as_ref(),
                &package.images,
                &self.options,
            );
        warnings.append(&mut mapper_warnings);

        // Set provenance (version detected above overrides any version the
        // mapper may have computed from the body XML).
        document.source = Some(
            DocumentSource::new("odf").with_version(source_version.as_str()),
        );

        Ok(OdtImportResult {
            document,
            warnings,
            source_version,
        })
    }
}

// ── Private helpers ────────────────────────────────────────────────────────────

/// Extract the raw value of the `office:version` attribute from `content.xml`
/// bytes without fully re-parsing; returns `None` if absent.
fn raw_version_attr(content: &[u8]) -> Option<String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_reader(content);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                let local = {
                    let b = e.local_name().into_inner();
                    if let Some(p) = b.iter().rposition(|&x| x == b':') {
                        b[p + 1..].to_vec()
                    } else {
                        b.to_vec()
                    }
                };
                if local == b"document-content" || local == b"document" {
                    return e.attributes().flatten().find_map(|attr| {
                        let key = attr.key.as_ref();
                        let key_local =
                            if let Some(p) =
                                key.iter().rposition(|&x| x == b':')
                            {
                                &key[p + 1..]
                            } else {
                                key
                            };
                        if key_local == b"version" {
                            attr.unescape_value()
                                .ok()
                                .map(std::borrow::Cow::into_owned)
                        } else {
                            None
                        }
                    });
                }
                buf.clear();
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => buf.clear(),
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::write::{FileOptions, ZipWriter};
    use zip::CompressionMethod;

    use super::*;
    use crate::constants::{
        ENTRY_CONTENT, ENTRY_MANIFEST, ENTRY_STYLES, MIME_ODT,
    };

    fn build_odt_zip(version: Option<&str>) -> Vec<u8> {
        let ver_attr = match version {
            Some(v) => format!(" office:version=\"{v}\""),
            None => String::new(),
        };
        let content = format!(
            r#"<?xml version="1.0"?><office:document-content{ver_attr}/>"#
        );

        let mut buf = Vec::new();
        let mut zip = ZipWriter::new(Cursor::new(&mut buf));

        let stored =
            FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
        zip.start_file("mimetype", stored).unwrap();
        zip.write_all(MIME_ODT.as_bytes()).unwrap();

        let deflated = FileOptions::<()>::default()
            .compression_method(CompressionMethod::Deflated);
        zip.start_file(ENTRY_MANIFEST, deflated).unwrap();
        zip.write_all(b"<manifest:manifest/>").unwrap();
        zip.start_file(ENTRY_CONTENT, deflated).unwrap();
        zip.write_all(content.as_bytes()).unwrap();
        zip.start_file(ENTRY_STYLES, deflated).unwrap();
        zip.write_all(b"<office:document-styles/>").unwrap();

        zip.finish().unwrap();
        buf
    }

    #[test]
    fn run_returns_source_version_1_2() {
        let zip = build_odt_zip(Some("1.2"));
        let result = OdtImporter::new(OdtImportOptions::default())
            .run(Cursor::new(zip))
            .unwrap();
        assert_eq!(result.source_version, OdfVersion::V1_2);
        assert_eq!(
            result.document.source.as_ref().unwrap().version.as_deref(),
            Some("1.2")
        );
    }

    #[test]
    fn run_absent_version_is_v1_1() {
        let zip = build_odt_zip(None);
        let result = OdtImporter::new(OdtImportOptions::default())
            .run(Cursor::new(zip))
            .unwrap();
        assert_eq!(result.source_version, OdfVersion::V1_1);
    }

    #[test]
    fn run_unknown_version_non_strict_emits_warning() {
        let zip = build_odt_zip(Some("99.0"));
        let result = OdtImporter::new(OdtImportOptions::default())
            .run(Cursor::new(zip))
            .unwrap();
        assert_eq!(result.source_version, OdfVersion::V1_3);
        assert!(
            result.warnings.iter().any(|w| matches!(
                w,
                OdfWarning::UnrecognisedVersion { version }
                    if version == "99.0"
            )),
            "expected UnrecognisedVersion warning"
        );
    }

    #[test]
    fn run_unknown_version_strict_returns_error() {
        let zip = build_odt_zip(Some("99.0"));
        let opts =
            OdtImportOptions { strict_version: true, ..Default::default() };
        let result = OdtImporter::new(opts).run(Cursor::new(zip));
        assert!(
            matches!(
                result,
                Err(OdfError::UnsupportedVersion { ref version })
                    if version == "99.0"
            ),
            "expected UnsupportedVersion error, got {result:?}"
        );
    }
}
