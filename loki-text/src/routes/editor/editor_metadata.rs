// SPDX-License-Identifier: Apache-2.0

//! Document metadata model for the Publish ribbon tab.
//!
//! Defines the editable Dublin Core fields ([`MetaField`]), the editor draft
//! ([`MetaDraft`]), and the conversion to/from the live document's
//! [`loki_doc_model::meta::DocumentMeta`]. The inline panel UI that consumes
//! these lives in `editor_metadata_panel`.

use std::sync::{Arc, Mutex};

use loki_doc_model::meta::LanguageTag;
use loki_i18n::fl;

use crate::editing::state::DocumentState;

/// A single editable Dublin Core field.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MetaField {
    Title,
    Creator,
    Subject,
    Description,
    Keywords,
    Language,
    Publisher,
    Contributors,
    Rights,
    License,
    Identifier,
    IdentifierScheme,
    DcType,
    Source,
    Relation,
    Coverage,
    Issued,
    Citation,
}

/// Display order of the metadata fields in the editor.
const FIELD_ORDER: [MetaField; 18] = [
    MetaField::Title,
    MetaField::Creator,
    MetaField::Subject,
    MetaField::Description,
    MetaField::Keywords,
    MetaField::Language,
    MetaField::Publisher,
    MetaField::Contributors,
    MetaField::Rights,
    MetaField::License,
    MetaField::Identifier,
    MetaField::IdentifierScheme,
    MetaField::DcType,
    MetaField::Source,
    MetaField::Relation,
    MetaField::Coverage,
    MetaField::Issued,
    MetaField::Citation,
];

impl MetaField {
    pub(super) fn label(self) -> String {
        match self {
            MetaField::Title => fl!("metadata-title"),
            MetaField::Creator => fl!("metadata-creator"),
            MetaField::Subject => fl!("metadata-subject"),
            MetaField::Description => fl!("metadata-description"),
            MetaField::Keywords => fl!("metadata-keywords"),
            MetaField::Language => fl!("metadata-language"),
            MetaField::Publisher => fl!("metadata-publisher"),
            MetaField::Contributors => fl!("metadata-contributors"),
            MetaField::Rights => fl!("metadata-rights"),
            MetaField::License => fl!("metadata-license"),
            MetaField::Identifier => fl!("metadata-identifier"),
            MetaField::IdentifierScheme => fl!("metadata-identifier-scheme"),
            MetaField::DcType => fl!("metadata-type"),
            MetaField::Source => fl!("metadata-source"),
            MetaField::Relation => fl!("metadata-relation"),
            MetaField::Coverage => fl!("metadata-coverage"),
            MetaField::Issued => fl!("metadata-issued"),
            MetaField::Citation => fl!("metadata-citation"),
        }
    }
}

/// Editable metadata snapshot: each field paired with its current string value.
#[derive(Clone, PartialEq)]
pub(super) struct MetaDraft {
    pub values: Vec<(MetaField, String)>,
}

/// Builds a [`MetaDraft`] from the currently loaded document's metadata.
pub(super) fn meta_to_draft(doc_state: &Arc<Mutex<DocumentState>>) -> MetaDraft {
    let guard = doc_state.lock().ok();
    let meta = guard
        .as_ref()
        .and_then(|s| s.document.as_ref())
        .map(|d| d.meta.clone())
        .unwrap_or_default();
    let dc = &meta.dublin_core;
    let opt = |o: &Option<String>| o.clone().unwrap_or_default();

    let values = FIELD_ORDER
        .iter()
        .map(|&field| {
            let v = match field {
                MetaField::Title => opt(&meta.title),
                MetaField::Creator => opt(&meta.creator),
                MetaField::Subject => opt(&meta.subject),
                MetaField::Description => opt(&meta.description),
                MetaField::Keywords => opt(&meta.keywords),
                MetaField::Language => meta
                    .language
                    .as_ref()
                    .map(|l| l.as_str().to_string())
                    .unwrap_or_default(),
                MetaField::Publisher => opt(&dc.publisher),
                MetaField::Contributors => dc.contributors.join("; "),
                MetaField::Rights => opt(&dc.rights),
                MetaField::License => opt(&dc.license),
                MetaField::Identifier => opt(&dc.identifier),
                MetaField::IdentifierScheme => opt(&dc.identifier_scheme),
                MetaField::DcType => opt(&dc.dc_type),
                MetaField::Source => opt(&dc.source),
                MetaField::Relation => opt(&dc.relation),
                MetaField::Coverage => opt(&dc.coverage),
                MetaField::Issued => opt(&dc.issued),
                MetaField::Citation => opt(&dc.bibliographic_citation),
            };
            (field, v)
        })
        .collect();
    MetaDraft { values }
}

/// Persists a [`MetaDraft`] into the document's metadata **through the Loro
/// CRDT**, so the change survives incremental rebuilds, participates in
/// undo/redo, and round-trips through Loro import/export.
///
/// The draft's editable fields are merged onto the current metadata (preserving
/// non-editable fields such as creation/modification dates and revision count),
/// then written to the metadata map. The caller is expected to follow with
/// `apply_mutation_and_relayout` so the live document is re-derived from Loro.
pub(super) fn apply_meta_draft(
    loro: &loro::LoroDoc,
    doc_state: &Arc<Mutex<DocumentState>>,
    draft: &MetaDraft,
) {
    // Start from the current metadata so non-edited fields are preserved.
    let mut meta = doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.as_ref().map(|d| d.meta.clone()))
        .unwrap_or_default();

    let some = |s: &str| {
        let t = s.trim();
        (!t.is_empty()).then(|| t.to_string())
    };
    for (field, value) in &draft.values {
        let dc = &mut meta.dublin_core;
        match field {
            MetaField::Title => meta.title = some(value),
            MetaField::Creator => meta.creator = some(value),
            MetaField::Subject => meta.subject = some(value),
            MetaField::Description => meta.description = some(value),
            MetaField::Keywords => meta.keywords = some(value),
            MetaField::Language => meta.language = some(value).map(LanguageTag::new),
            MetaField::Publisher => dc.publisher = some(value),
            MetaField::Contributors => {
                dc.contributors = value
                    .split(';')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
            }
            MetaField::Rights => dc.rights = some(value),
            MetaField::License => dc.license = some(value),
            MetaField::Identifier => dc.identifier = some(value),
            MetaField::IdentifierScheme => dc.identifier_scheme = some(value),
            MetaField::DcType => dc.dc_type = some(value),
            MetaField::Source => dc.source = some(value),
            MetaField::Relation => dc.relation = some(value),
            MetaField::Coverage => dc.coverage = some(value),
            MetaField::Issued => dc.issued = some(value),
            MetaField::Citation => dc.bibliographic_citation = some(value),
        }
    }

    if let Err(e) = loki_doc_model::loro_bridge::write_document_meta(loro, &meta) {
        tracing::warn!("failed to persist metadata to Loro: {e}");
    }
    loro.commit();
}
