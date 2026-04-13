// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Provenance information about the document source format.
//!
//! When a [`crate::Document`] is loaded from a file, [`DocumentSource`]
//! records which format and version it came from. This allows exporters
//! to make format-version-aware decisions.

/// Provenance of a document loaded from a file.
///
/// Populated by format-specific importers (`loki-odf`, `loki-ooxml`).
/// `None` for documents constructed programmatically.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DocumentSource {
    /// A short string identifying the source format.
    ///
    /// Conventional values:
    /// - `"odf"` — Open Document Format
    /// - `"ooxml"` — Office Open XML (ISO/IEC 29500)
    /// - `"doc"` — legacy Word binary format (`.doc`)
    pub format: String,

    /// The specific format version, if determinable.
    ///
    /// Examples:
    /// - ODF: `"1.2"`, `"1.3"`
    /// - OOXML: `"strict"`, `"transitional"`
    pub version: Option<String>,

    /// The application that produced the source file, if present in the
    /// document metadata.
    ///
    /// ODF: `meta:generator`. OOXML: `AppVersion` in `app.xml`.
    pub generator: Option<String>,
}

impl DocumentSource {
    /// Creates a new [`DocumentSource`] with the given format identifier.
    #[must_use]
    pub fn new(format: impl Into<String>) -> Self {
        Self {
            format: format.into(),
            version: None,
            generator: None,
        }
    }

    /// Builder: set the format version.
    #[must_use]
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Builder: set the generator application string.
    #[must_use]
    pub fn with_generator(mut self, generator: impl Into<String>) -> Self {
        self.generator = Some(generator.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_chain() {
        let src = DocumentSource::new("odf")
            .with_version("1.3")
            .with_generator("LibreOffice Writer");
        assert_eq!(src.format, "odf");
        assert_eq!(src.version.as_deref(), Some("1.3"));
        assert_eq!(src.generator.as_deref(), Some("LibreOffice Writer"));
    }
}
