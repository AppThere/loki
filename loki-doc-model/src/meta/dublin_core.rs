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

    /// Flattens the set fields into `(name, value)` pairs under reserved
    /// `dcmi:` names, in a stable order.
    ///
    /// This is the canonical flattening used by formats that have no native
    /// element for these fields: OOXML carries them as `docProps/custom.xml`
    /// custom properties, ODF as `meta:user-defined` entries. Repeatable
    /// `contributors` become `dcmi:contributor.{i}`. The inverse is
    /// [`Self::from_named_pairs`].
    #[must_use]
    pub fn to_named_pairs(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        let mut push = |name: &str, value: &Option<String>| {
            if let Some(v) = value {
                pairs.push((name.to_string(), v.clone()));
            }
        };
        push(DC_PUBLISHER, &self.publisher);
        push(DC_RIGHTS, &self.rights);
        push(DC_LICENSE, &self.license);
        push(DC_IDENTIFIER, &self.identifier);
        push(DC_IDENTIFIER_SCHEME, &self.identifier_scheme);
        push(DC_TYPE, &self.dc_type);
        push(DC_FORMAT, &self.format);
        push(DC_SOURCE, &self.source);
        push(DC_RELATION, &self.relation);
        push(DC_COVERAGE, &self.coverage);
        push(DC_ISSUED, &self.issued);
        push(DC_BIBLIOGRAPHIC_CITATION, &self.bibliographic_citation);
        for (i, c) in self.contributors.iter().enumerate() {
            pairs.push((format!("{DC_CONTRIBUTOR_PREFIX}{i}"), c.clone()));
        }
        pairs
    }

    /// Rebuilds the fields from `(name, value)` pairs produced by
    /// [`Self::to_named_pairs`]. Unknown names are ignored; `contributors`
    /// are ordered by their numeric suffix.
    #[must_use]
    pub fn from_named_pairs(pairs: &[(String, String)]) -> Self {
        let mut dc = Self::default();
        let mut contributors: Vec<(usize, String)> = Vec::new();
        for (name, value) in pairs {
            let v = || Some(value.clone());
            match name.as_str() {
                DC_PUBLISHER => dc.publisher = v(),
                DC_RIGHTS => dc.rights = v(),
                DC_LICENSE => dc.license = v(),
                DC_IDENTIFIER => dc.identifier = v(),
                DC_IDENTIFIER_SCHEME => dc.identifier_scheme = v(),
                DC_TYPE => dc.dc_type = v(),
                DC_FORMAT => dc.format = v(),
                DC_SOURCE => dc.source = v(),
                DC_RELATION => dc.relation = v(),
                DC_COVERAGE => dc.coverage = v(),
                DC_ISSUED => dc.issued = v(),
                DC_BIBLIOGRAPHIC_CITATION => dc.bibliographic_citation = v(),
                other => {
                    if let Some(idx) = other
                        .strip_prefix(DC_CONTRIBUTOR_PREFIX)
                        .and_then(|n| n.parse::<usize>().ok())
                    {
                        contributors.push((idx, value.clone()));
                    }
                }
            }
        }
        contributors.sort_by_key(|(i, _)| *i);
        dc.contributors = contributors.into_iter().map(|(_, c)| c).collect();
        dc
    }
}

// Reserved `dcmi:` property names shared by the OOXML and ODF metadata writers.
const DC_PUBLISHER: &str = "dcmi:publisher";
const DC_RIGHTS: &str = "dcmi:rights";
const DC_LICENSE: &str = "dcmi:license";
const DC_IDENTIFIER: &str = "dcmi:identifier";
const DC_IDENTIFIER_SCHEME: &str = "dcmi:identifier-scheme";
const DC_TYPE: &str = "dcmi:type";
const DC_FORMAT: &str = "dcmi:format";
const DC_SOURCE: &str = "dcmi:source";
const DC_RELATION: &str = "dcmi:relation";
const DC_COVERAGE: &str = "dcmi:coverage";
const DC_ISSUED: &str = "dcmi:issued";
const DC_BIBLIOGRAPHIC_CITATION: &str = "dcmi:bibliographic-citation";
const DC_CONTRIBUTOR_PREFIX: &str = "dcmi:contributor.";

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
