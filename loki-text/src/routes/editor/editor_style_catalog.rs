// SPDX-License-Identifier: Apache-2.0

//! Style catalog mutation helpers for the document editor.

use std::sync::{Arc, Mutex};

use loki_doc_model::style::ParagraphStyle;
use loki_doc_model::style::catalog::StyleId;

use crate::editing::state::DocumentState;

/// Inserts or replaces a style in the document's style catalog.
pub(super) fn upsert_catalog_style(doc_state: &Arc<Mutex<DocumentState>>, style: ParagraphStyle) {
    let Ok(mut state) = doc_state.lock() else {
        return;
    };
    let Some(arc_doc) = state.document.as_mut() else {
        return;
    };
    let doc = Arc::make_mut(arc_doc);
    doc.styles.paragraph_styles.insert(style.id.clone(), style);
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
/// Enumerates the editor's shared Fontique collection. Intended to be called
/// once (memoised) per editor rather than per render.
pub(super) fn available_font_families(doc_state: &Arc<Mutex<DocumentState>>) -> Vec<String> {
    let Ok(state) = doc_state.lock() else {
        return vec![];
    };
    let Ok(mut fr) = state.shared_font_resources.lock() else {
        return vec![];
    };
    fr.available_font_families()
}

/// Returns `(style_id, display_name)` pairs for all catalog styles, sorted by display name.
pub(super) fn catalog_style_list(doc_state: &Arc<Mutex<DocumentState>>) -> Vec<(String, String)> {
    let Ok(state) = doc_state.lock() else {
        return vec![];
    };
    let Some(doc) = &state.document else {
        return vec![];
    };
    let mut entries: Vec<(String, String)> = doc
        .styles
        .paragraph_styles
        .iter()
        .map(|(id, style)| {
            let display = style
                .display_name
                .clone()
                .unwrap_or_else(|| id.as_str().to_string());
            (id.as_str().to_string(), display)
        })
        .collect();
    entries.sort_by(|(_, a), (_, b)| a.cmp(b));
    entries
}
