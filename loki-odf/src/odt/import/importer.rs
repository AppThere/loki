// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`OdtImport`] and [`OdtImporter`] — the primary public API for ODT import.

use std::io::{Read, Seek};

use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentImport;
use loki_doc_model::io::source::DocumentSource;

use crate::error::{OdfError, OdfResult, OdfWarning};
use crate::package::OdfPackage;
use crate::version::OdfVersion;

use super::options::OdtImportOptions;
use super::types::OdtImportResult;
use super::version_attr::raw_version_attr;

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
    fn import(reader: impl Read + Seek, options: Self::Options) -> OdfResult<Document> {
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
    pub fn run(self, reader: impl Read + Seek) -> OdfResult<OdtImportResult> {
        let mut warnings: Vec<OdfWarning> = Vec::new();

        let package = OdfPackage::open(reader)?;

        // ── Version detection ─────────────────────────────────────────────
        // If the version was absent and strict mode is off, that is valid
        // (ODF 1.1). If it was present but unrecognised and strict mode is
        // on, raise an error.
        let source_version = if !package.version_was_absent && package.version == OdfVersion::V1_3 {
            // detect_version returns V1_3 as a fallback for unknown strings;
            // re-examine the raw attribute to surface an
            // UnrecognisedVersion warning when appropriate.
            let raw = raw_version_attr(&package.content);
            match raw {
                Some(ref s) if OdfVersion::from_attr(s).is_none() => {
                    if self.options.strict_version {
                        return Err(OdfError::UnsupportedVersion { version: s.clone() });
                    }
                    warnings.push(OdfWarning::UnrecognisedVersion { version: s.clone() });
                    OdfVersion::V1_3
                }
                _ => package.version,
            }
        } else {
            package.version
        };

        // ── Parse XML parts ───────────────────────────────────────────────
        let odf_doc = crate::odt::reader::document::read_document(&package.content)?;

        let mut stylesheet = crate::odt::reader::styles::read_stylesheet(&package.styles, false)?;

        // Merge automatic styles from content.xml (paragraph/span-level styles
        // that are specific to this document instance).
        let auto_styles = crate::odt::reader::styles::read_auto_styles(&package.content)?;
        stylesheet.merge_auto(auto_styles);

        let odf_meta = package
            .meta
            .as_deref()
            .map(crate::odt::reader::meta::read_meta)
            .transpose()?;

        // ── Map to document model ─────────────────────────────────────────
        let (mut document, mut mapper_warnings) = crate::odt::mapper::document::map_document(
            &odf_doc,
            &stylesheet,
            odf_meta.as_ref(),
            &package.images,
            &self.options,
        );
        warnings.append(&mut mapper_warnings);

        // Set provenance (version detected above overrides any version the
        // mapper may have computed from the body XML).
        document.source = Some(DocumentSource::new("odf").with_version(source_version.as_str()));

        Ok(OdtImportResult {
            document,
            warnings,
            source_version,
        })
    }
}
