// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Dublin Core metadata: the DCMI Metadata Element Set and selected Terms.
//!
//! [`DublinCoreMeta`] captures the Dublin Core fields that are relevant to
//! publishing (PDF/X and EPUB 3.3 export) but are **not** already present on
//! [`crate::meta::DocumentMeta`]. The shared fields — `title`, `creator`,
//! `subject`, `description`, `language`, `keywords`, `created`, `modified` —
//! remain on `DocumentMeta` and are not duplicated here so there is a single
//! source of truth.
//!
//! References:
//! - DCMI Metadata Terms — <https://www.dublincore.org/specifications/dublin-core/dcmi-terms/>
//! - EPUB 3.3 §5.4 (Package metadata) maps `dc:*` elements directly.
//! - PDF/X (ISO 15930) XMP packets carry the same fields via the `dc:` schema.
//!
//! The fifteen DCMES elements are: `contributor`, `coverage`, `creator`,
//! `date`, `description`, `format`, `identifier`, `language`, `publisher`,
//! `relation`, `rights`, `source`, `subject`, `title`, `type`.  Elements
//! already modelled on `DocumentMeta` are annotated below.

/// The DCMI Type Vocabulary value for a textual resource.
///
/// EPUB and most word-processor documents are `Text`.  Exposed as a constant
/// so the export pipelines and the metadata editor share one default.
pub const DCMI_TYPE_TEXT: &str = "Text";

/// Dublin Core metadata fields used by the publishing exporters.
///
/// Only fields **not** already on [`crate::meta::DocumentMeta`] live here.
/// All fields are optional or empty by default; an all-default value
/// contributes no `dc:*` elements to the exported package.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DublinCoreMeta {
    /// `dc:contributor` — entities that made contributions other than the
    /// primary creator (editors, illustrators, translators, …).  Repeatable.
    pub contributors: Vec<String>,

    /// `dc:publisher` — the entity responsible for making the resource
    /// available (publishing house, imprint, or self).
    pub publisher: Option<String>,

    /// `dc:rights` — a statement about rights held in and over the resource
    /// (e.g. a copyright line).
    pub rights: Option<String>,

    /// `dcterms:license` — a legal document giving official permission to do
    /// something with the resource (e.g. a Creative Commons URL).
    pub license: Option<String>,

    /// `dc:identifier` — an unambiguous reference such as an ISBN, DOI, or
    /// UUID.  Required by EPUB 3.3; a UUID is synthesised at export time when
    /// this is empty.
    pub identifier: Option<String>,

    /// The scheme of [`Self::identifier`] (e.g. `"ISBN"`, `"DOI"`, `"UUID"`).
    /// Emitted as the EPUB `<dc:identifier>` refinement
    /// `dcterms:`-style `identifier-type` where supported.
    pub identifier_scheme: Option<String>,

    /// `dc:type` — the nature or genre of the resource, ideally from the DCMI
    /// Type Vocabulary.  Defaults to [`DCMI_TYPE_TEXT`] at export when empty.
    pub dc_type: Option<String>,

    /// `dc:format` — the file format or medium (a MIME type for digital
    /// resources, e.g. `application/pdf` or `application/epub+zip`).
    pub format: Option<String>,

    /// `dc:source` — a related resource from which the described resource is
    /// derived.
    pub source: Option<String>,

    /// `dc:relation` — a related resource.
    pub relation: Option<String>,

    /// `dc:coverage` — the spatial or temporal topic, jurisdiction, or
    /// applicability of the resource.
    pub coverage: Option<String>,

    /// `dcterms:issued` — the date of formal issuance (publication) of the
    /// resource, as an ISO-8601 date string (`YYYY` or `YYYY-MM-DD`).  EPUB 3.3
    /// recommends a plain date for `dcterms:date`.
    pub issued: Option<String>,

    /// `dcterms:bibliographicCitation` — a bibliographic reference for the
    /// resource (preferred citation).
    pub bibliographic_citation: Option<String>,
}

impl DublinCoreMeta {
    /// Returns `true` when no Dublin Core field carries a value.
    ///
    /// Used by exporters to skip emitting an empty extended-metadata block.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.contributors.is_empty()
            && self.publisher.is_none()
            && self.rights.is_none()
            && self.license.is_none()
            && self.identifier.is_none()
            && self.identifier_scheme.is_none()
            && self.dc_type.is_none()
            && self.format.is_none()
            && self.source.is_none()
            && self.relation.is_none()
            && self.coverage.is_none()
            && self.issued.is_none()
            && self.bibliographic_citation.is_none()
    }

    /// Returns the resource type, falling back to [`DCMI_TYPE_TEXT`].
    #[must_use]
    pub fn dc_type_or_default(&self) -> &str {
        self.dc_type.as_deref().unwrap_or(DCMI_TYPE_TEXT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let dc = DublinCoreMeta::default();
        assert!(dc.is_empty());
        assert_eq!(dc.dc_type_or_default(), DCMI_TYPE_TEXT);
    }

    #[test]
    fn non_empty_when_publisher_set() {
        let dc = DublinCoreMeta {
            publisher: Some("AppThere Press".into()),
            ..Default::default()
        };
        assert!(!dc.is_empty());
    }

    #[test]
    fn non_empty_when_contributor_present() {
        let dc = DublinCoreMeta {
            contributors: vec!["Editor Name".into()],
            ..Default::default()
        };
        assert!(!dc.is_empty());
    }

    #[test]
    fn dc_type_override_is_returned() {
        let dc = DublinCoreMeta {
            dc_type: Some("Image".into()),
            ..Default::default()
        };
        assert_eq!(dc.dc_type_or_default(), "Image");
    }
}
