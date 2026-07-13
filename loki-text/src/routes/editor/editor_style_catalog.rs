// SPDX-License-Identifier: Apache-2.0

//! Style catalog mutation helpers for the document editor.

use std::sync::{Arc, Mutex};

use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::{CharacterStyle, ParagraphStyle};

use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Why a style delete was refused.
pub(super) enum DeleteError {
    /// Built-in / default styles are protected from deletion (Spec 05 §8).
    Builtin,
    /// The style id is not in the catalog (or no document is loaded).
    NotFound,
}

/// Deletes the paragraph style `id` (re-parenting its children to the
/// grandparent), persists the catalog, and relays out. Built-in styles are
/// refused. On success returns the number of re-parented child styles for the
/// caller's confirmation message. The caller refreshes undo bookkeeping.
pub(super) fn perform_style_delete(
    loro: &loro::LoroDoc,
    doc_state: &Arc<Mutex<DocumentState>>,
    id: &str,
) -> Result<usize, DeleteError> {
    let mut catalog = catalog_snapshot(doc_state).ok_or(DeleteError::NotFound)?;
    let sid = StyleId::new(id);
    let style = catalog
        .paragraph_styles
        .get(&sid)
        .ok_or(DeleteError::NotFound)?;
    if style.is_builtin() {
        return Err(DeleteError::Builtin);
    }
    let reparented = catalog.delete_paragraph_style(&sid).len();
    if let Err(e) = loki_doc_model::loro_bridge::write_document_styles(loro, &catalog) {
        tracing::warn!("failed to persist style catalog to Loro: {e}");
    }
    loro.commit();
    apply_mutation_and_relayout(doc_state, loro);
    Ok(reparented)
}

/// Clones the document's current [`StyleCatalog`] for read-only inspection
/// (e.g. the provenance inspector). Returns `None` when no document is loaded.
pub(super) fn catalog_snapshot(doc_state: &Arc<Mutex<DocumentState>>) -> Option<StyleCatalog> {
    doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.as_ref().map(|d| d.styles.clone()))
}

/// Inserts or replaces a paragraph style in the catalog and persists the result
/// through the Loro CRDT, committing it as a discrete, undoable transaction.
///
/// The catalog is the Loro snapshot's responsibility (see `loro_bridge::styles`),
/// so the edit is written there rather than mutated in place on `state.document`
/// — the subsequent `apply_mutation_and_relayout` re-derives the catalog from
/// Loro. Starting from the current catalog preserves every other style. The
/// caller refreshes undo bookkeeping via `post_mutation_sync`.
pub(super) fn commit_style_to_loro(
    loro: &loro::LoroDoc,
    doc_state: &Arc<Mutex<DocumentState>>,
    style: ParagraphStyle,
) {
    let mut catalog = doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.as_ref().map(|d| d.styles.clone()))
        .unwrap_or_default();
    catalog.paragraph_styles.insert(style.id.clone(), style);
    if let Err(e) = loki_doc_model::loro_bridge::write_document_styles(loro, &catalog) {
        tracing::warn!("failed to persist style catalog to Loro: {e}");
    }
    loro.commit();
}

/// Persists an edited `CharacterStyle` into the catalog through Loro — the
/// character-family analogue of [`commit_style_to_loro`] (Spec 05 M6). Inserts
/// (or replaces) `style` in `character_styles` and writes the whole catalog back
/// as a discrete, undoable CRDT transaction. The caller relays out and refreshes
/// undo bookkeeping.
pub(super) fn commit_char_style_to_loro(
    loro: &loro::LoroDoc,
    doc_state: &Arc<Mutex<DocumentState>>,
    style: CharacterStyle,
) {
    let mut catalog = doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.as_ref().map(|d| d.styles.clone()))
        .unwrap_or_default();
    catalog.character_styles.insert(style.id.clone(), style);
    if let Err(e) = loki_doc_model::loro_bridge::write_document_styles(loro, &catalog) {
        tracing::warn!("failed to persist style catalog to Loro: {e}");
    }
    loro.commit();
}

/// Persists an edited `TableStyle` into the catalog through Loro — the table
/// family's analogue of [`commit_char_style_to_loro`] (Spec 05 M6, 4a.3).
pub(super) fn commit_table_style_to_loro(
    loro: &loro::LoroDoc,
    doc_state: &Arc<Mutex<DocumentState>>,
    style: loki_doc_model::style::table_style::TableStyle,
) {
    let mut catalog = doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.as_ref().map(|d| d.styles.clone()))
        .unwrap_or_default();
    catalog.table_styles.insert(style.id.clone(), style);
    if let Err(e) = loki_doc_model::loro_bridge::write_document_styles(loro, &catalog) {
        tracing::warn!("failed to persist style catalog to Loro: {e}");
    }
    loro.commit();
}

/// Returns a clone of the catalog **table** style with the given id, or `None`.
pub(super) fn get_catalog_table_style(
    doc_state: &Arc<Mutex<DocumentState>>,
    style_id: &str,
) -> Option<loki_doc_model::style::table_style::TableStyle> {
    let state = doc_state.lock().ok()?;
    let doc = state.document.as_ref()?;
    doc.styles
        .table_styles
        .get(&loki_doc_model::style::StyleId::new(style_id))
        .cloned()
}

/// Returns a clone of the catalog **character** style with the given id, or `None`.
pub(super) fn get_catalog_char_style(
    doc_state: &Arc<Mutex<DocumentState>>,
    style_id: &str,
) -> Option<CharacterStyle> {
    let state = doc_state.lock().ok()?;
    let doc = state.document.as_ref()?;
    doc.styles
        .character_styles
        .get(&StyleId::new(style_id))
        .cloned()
}

/// Clears the local override of `property` on the paragraph style `id` and
/// persists the result through Loro (a discrete, undoable transaction) — the
/// "reset to inherited" action (Spec 05 §6). A no-op when the style is absent.
///
/// The caller relays out and refreshes undo bookkeeping (as for
/// [`commit_style_to_loro`]).
pub(super) fn reset_style_property(
    loro: &loro::LoroDoc,
    doc_state: &Arc<Mutex<DocumentState>>,
    id: &str,
    property: super::style_inspector::StyleProperty,
) {
    if let Some(mut style) = get_catalog_style(doc_state, id) {
        super::style_inspector::clear_local_property(&mut style, property);
        commit_style_to_loro(loro, doc_state, style);
    }
}

/// Generates a unique id string for a new custom style.
pub(super) fn new_custom_style_id(doc_state: &Arc<Mutex<DocumentState>>) -> String {
    let Ok(state) = doc_state.lock() else {
        return "CustomStyle1".to_string();
    };
    let Some(doc) = &state.document else {
        return "CustomStyle1".to_string();
    };
    for n in 1_u32..=9999 {
        let candidate = format!("CustomStyle{n}");
        if !doc
            .styles
            .paragraph_styles
            .contains_key(&StyleId::new(&candidate))
        {
            return candidate;
        }
    }
    "CustomStyle9999".to_string()
}

/// Returns a clone of the catalog style with the given id, or `None`.
pub(super) fn get_catalog_style(
    doc_state: &Arc<Mutex<DocumentState>>,
    style_id: &str,
) -> Option<ParagraphStyle> {
    let state = doc_state.lock().ok()?;
    let doc = state.document.as_ref()?;
    doc.styles
        .paragraph_styles
        .get(&StyleId::new(style_id))
        .cloned()
}

/// Returns the font families available for layout (system + bundled +
/// document-embedded), sorted, for the style editor's font picker.
///
/// Blocks until the background font warm-up finishes, so call it from a
/// worker thread (the editor fills its `font_families` signal that way)
/// rather than on the render path.
pub(super) fn available_font_families(fonts: &loki_layout::SharedFontResources) -> Vec<String> {
    fonts.lock().available_font_families()
}

/// Returns `(style_id, display_name, depth)` for all catalog paragraph styles in
/// inheritance-tree pre-order (parents before their subtrees), for the tree-view
/// picker (Spec 05 §7). `depth` is the indentation level.
pub(super) fn catalog_style_tree(
    doc_state: &Arc<Mutex<DocumentState>>,
) -> Vec<(String, String, usize)> {
    let Some(catalog) = catalog_snapshot(doc_state) else {
        return vec![];
    };
    catalog
        .para_forest_preorder()
        .into_iter()
        .map(|(id, depth)| {
            let display = catalog
                .paragraph_styles
                .get(&id)
                .and_then(|s| s.display_name.clone())
                .unwrap_or_else(|| id.as_str().to_string());
            (id.as_str().to_string(), display, depth)
        })
        .collect()
}
