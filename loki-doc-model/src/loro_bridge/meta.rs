// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Document metadata round-trip through the Loro CRDT.
//!
//! The full [`DocumentMeta`] — core properties plus the Dublin Core extension
//! ([`crate::meta::DublinCoreMeta`]) — is stored as a single JSON string under
//! [`KEY_META_JSON`] in the metadata map. This mirrors the opaque-block
//! strategy ([`super::opaque`]): a lossless snapshot via the model's `serde`
//! derives, rather than a hand-written field-by-field mapping that would have
//! to grow with every new metadata field.
//!
//! A plain `title` mirror is also written under [`KEY_META_TITLE`] so external
//! tooling (and the pre-existing read path) can find the title without parsing
//! the JSON.

use loro::{LoroDoc, LoroMap};

use super::BridgeError;
use crate::loro_schema::{KEY_META_JSON, KEY_META_TITLE, KEY_METADATA};
use crate::meta::DocumentMeta;

/// Writes `meta` into the metadata map of `map`.
pub(super) fn write_meta(meta: &DocumentMeta, map: &LoroMap) -> Result<(), BridgeError> {
    if let Some(title) = &meta.title {
        map.insert(KEY_META_TITLE, title.as_str())?;
    }
    write_meta_json(meta, map);
    Ok(())
}

#[cfg(feature = "serde")]
fn write_meta_json(meta: &DocumentMeta, map: &LoroMap) {
    match serde_json::to_string(meta) {
        Ok(json) => {
            if let Err(err) = map.insert(KEY_META_JSON, json) {
                tracing::warn!("loro bridge: failed to store metadata snapshot: {err}");
            }
        }
        // Unreachable in practice: DocumentMeta derives Serialize.
        Err(err) => tracing::warn!("loro bridge: failed to serialize metadata: {err}"),
    }
}

#[cfg(not(feature = "serde"))]
fn write_meta_json(_meta: &DocumentMeta, _map: &LoroMap) {
    tracing::warn!(
        "loro bridge: metadata not persisted — build loki-doc-model with the \
         `serde` feature (default) to round-trip document metadata"
    );
}

/// Reads [`DocumentMeta`] from the metadata map of `map`.
///
/// Falls back to the plain `title` mirror when the JSON snapshot is missing
/// (e.g. a document written before metadata round-tripping existed) and to
/// [`DocumentMeta::default`] when neither is present.
pub(super) fn read_meta(map: &LoroMap) -> DocumentMeta {
    if let Some(meta) = read_meta_json(map) {
        return meta;
    }
    let mut meta = DocumentMeta::default();
    if let Some(title) = map
        .get(KEY_META_TITLE)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
    {
        meta.title = Some(title.to_string());
    }
    meta
}

#[cfg(feature = "serde")]
fn read_meta_json(map: &LoroMap) -> Option<DocumentMeta> {
    let json = map
        .get(KEY_META_JSON)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())?;
    serde_json::from_str(&json).ok()
}

#[cfg(not(feature = "serde"))]
fn read_meta_json(_map: &LoroMap) -> Option<DocumentMeta> {
    None
}

/// Overwrites the document-level metadata stored in `loro` with `meta`.
///
/// Used by the editor to persist Publish-tab metadata edits as a CRDT
/// mutation, so they survive incremental rebuilds, undo/redo, and Loro
/// import/export. The caller is responsible for committing.
pub fn write_document_meta(loro: &LoroDoc, meta: &DocumentMeta) -> Result<(), BridgeError> {
    let map = loro.get_map(KEY_METADATA);
    write_meta(meta, &map)
}

/// Reads the document-level metadata stored in `loro`.
#[must_use]
pub fn read_document_meta(loro: &LoroDoc) -> DocumentMeta {
    let map = loro.get_map(KEY_METADATA);
    read_meta(&map)
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;
    use crate::meta::{DublinCoreMeta, LanguageTag};

    fn sample_meta() -> DocumentMeta {
        DocumentMeta {
            title: Some("Round Trip".into()),
            creator: Some("Ada".into()),
            language: Some(LanguageTag::new("en-GB")),
            dublin_core: DublinCoreMeta {
                publisher: Some("AppThere Press".into()),
                contributors: vec!["Editor One".into(), "Editor Two".into()],
                identifier: Some("urn:isbn:123".into()),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn round_trips_full_metadata() {
        let loro = LoroDoc::new();
        write_document_meta(&loro, &sample_meta()).expect("write");
        loro.commit();
        let read = read_document_meta(&loro);
        assert_eq!(read, sample_meta());
    }

    #[test]
    fn title_mirror_is_written() {
        let loro = LoroDoc::new();
        write_document_meta(&loro, &sample_meta()).expect("write");
        loro.commit();
        let map = loro.get_map(KEY_METADATA);
        let title = map
            .get(KEY_META_TITLE)
            .and_then(|v| v.into_value().ok())
            .and_then(|v| v.into_string().ok())
            .map(|s| s.to_string());
        assert_eq!(title.as_deref(), Some("Round Trip"));
    }

    #[test]
    fn missing_snapshot_falls_back_to_default() {
        let loro = LoroDoc::new();
        let read = read_document_meta(&loro);
        assert_eq!(read, DocumentMeta::default());
    }
}
