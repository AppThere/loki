// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Style catalog round-trip through the Loro CRDT.
//!
//! The whole [`StyleCatalog`] — paragraph, character, table, and list styles —
//! is stored as a single JSON string under [`KEY_STYLE_CATALOG_JSON`] in the
//! style-catalog map. This mirrors the metadata strategy ([`super::meta`]): a
//! lossless snapshot via the model's `serde` derives, rather than a
//! hand-written field-by-field CRDT mapping that would have to grow with every
//! new style property.
//!
//! Storing the catalog in the CRDT (rather than carrying it forward by cloning
//! the previous [`Document`]) makes style-editor edits durable across
//! incremental rebuilds and — crucially — **undoable**: an edit is a committed
//! Loro transaction, so `UndoManager` reverts it like any text edit.

use loro::{LoroDoc, LoroMap};

use super::BridgeError;
use crate::loro_schema::{KEY_STYLE_CATALOG, KEY_STYLE_CATALOG_JSON};
use crate::style::catalog::StyleCatalog;

/// Writes `catalog` into the style-catalog `map` as a JSON snapshot.
pub(super) fn write_styles(catalog: &StyleCatalog, map: &LoroMap) -> Result<(), BridgeError> {
    write_styles_json(catalog, map);
    Ok(())
}

#[cfg(feature = "serde")]
fn write_styles_json(catalog: &StyleCatalog, map: &LoroMap) {
    match serde_json::to_string(catalog) {
        Ok(json) => {
            if let Err(err) = map.insert(KEY_STYLE_CATALOG_JSON, json) {
                tracing::warn!("loro bridge: failed to store style catalog snapshot: {err}");
            }
        }
        // Unreachable in practice: StyleCatalog derives Serialize.
        Err(err) => tracing::warn!("loro bridge: failed to serialize style catalog: {err}"),
    }
}

#[cfg(not(feature = "serde"))]
fn write_styles_json(_catalog: &StyleCatalog, _map: &LoroMap) {
    tracing::warn!(
        "loro bridge: style catalog not persisted — build loki-doc-model with the \
         `serde` feature (default) to round-trip styles"
    );
}

/// Reads the [`StyleCatalog`] from the style-catalog `map`.
///
/// Falls back to an empty catalog when the snapshot is missing (e.g. a document
/// written before style round-tripping existed, or one with no styles).
pub(super) fn read_styles(map: &LoroMap) -> StyleCatalog {
    read_styles_json(map).unwrap_or_default()
}

#[cfg(feature = "serde")]
fn read_styles_json(map: &LoroMap) -> Option<StyleCatalog> {
    let json = map
        .get(KEY_STYLE_CATALOG_JSON)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())?;
    serde_json::from_str(&json).ok()
}

#[cfg(not(feature = "serde"))]
fn read_styles_json(_map: &LoroMap) -> Option<StyleCatalog> {
    None
}

/// Overwrites the style catalog stored in `loro` with `catalog`.
///
/// Used by the editor to persist style-editor edits as a CRDT mutation, so they
/// survive incremental rebuilds, undo/redo, and Loro import/export. The caller
/// is responsible for committing.
pub fn write_document_styles(loro: &LoroDoc, catalog: &StyleCatalog) -> Result<(), BridgeError> {
    let map = loro.get_map(KEY_STYLE_CATALOG);
    write_styles(catalog, &map)
}

/// Reads the style catalog stored in `loro`.
#[must_use]
pub fn read_document_styles(loro: &LoroDoc) -> StyleCatalog {
    let map = loro.get_map(KEY_STYLE_CATALOG);
    read_styles(&map)
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;
    use crate::style::catalog::StyleId;
    use crate::style::para_style::ParagraphStyle;
    use crate::style::props::char_props::CharProps;
    use crate::style::props::para_props::{ParaProps, ParagraphAlignment};
    use loki_primitives::units::Points;

    fn sample_catalog() -> StyleCatalog {
        let mut catalog = StyleCatalog::new();
        let style = ParagraphStyle {
            id: StyleId::new("MyQuote"),
            display_name: Some("My Quote".into()),
            parent: Some(StyleId::new("Normal")),
            linked_char_style: None,
            next_style_id: Some("Normal".into()),
            para_props: ParaProps {
                alignment: Some(ParagraphAlignment::Justify),
                indent_start: Some(Points::new(36.0)),
                ..Default::default()
            },
            char_props: CharProps {
                font_name: Some("Arial".into()),
                bold: Some(true),
                font_weight: Some(700),
                ..Default::default()
            },
            is_default: false,
            is_custom: true,
            extensions: Default::default(),
        };
        catalog
            .paragraph_styles
            .insert(StyleId::new("MyQuote"), style);
        catalog
    }

    #[test]
    fn round_trips_catalog() {
        let loro = LoroDoc::new();
        let catalog = sample_catalog();
        write_document_styles(&loro, &catalog).expect("write");
        loro.commit();
        let read = read_document_styles(&loro);
        let original = catalog.paragraph_styles.get(&StyleId::new("MyQuote"));
        let restored = read.paragraph_styles.get(&StyleId::new("MyQuote"));
        assert_eq!(restored, original);
        assert!(restored.is_some_and(|s| s.is_custom));
    }

    #[test]
    fn missing_snapshot_falls_back_to_empty() {
        let loro = LoroDoc::new();
        let read = read_document_styles(&loro);
        assert!(read.paragraph_styles.is_empty());
    }
}
