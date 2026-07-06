// SPDX-License-Identifier: Apache-2.0

//! Pre-render inspector data for the style editor panel (Spec 05 M2/M4/M6).
//!
//! Extracted from `mod.rs` to keep that file under the 300-line ceiling. Given
//! the editor's `doc_state` and the current draft, [`inspector_data`] computes
//! everything the provenance column renders: the staged rows, the impact
//! preview, the default parent for a new style, and — for the **linked** family
//! (§9) — the paragraph style's linked character style rows.

use std::sync::{Arc, Mutex};

use loki_doc_model::style::{StyleId, derive_page_styles};

use super::super::editor_state::StyleDraft;
use super::super::editor_style_catalog::catalog_snapshot;
use super::super::style_char_inspector::character_inspector_rows;
use super::super::style_impact::affected_dependents;
use super::super::style_inspector::{InspectorRow, paragraph_inspector_rows};
use super::super::style_list_inspector::{ListLevelRow, list_inspector_rows};
use super::super::style_page_inspector::{PagePropRow, page_inspector_rows};
use super::draft::draft_to_style;
use crate::editing::state::DocumentState;

/// Everything the provenance column needs, derived once per render.
pub(super) struct InspectorData {
    /// Paragraph rows previewing the pending draft; the flag is `true` when the
    /// draft's uncommitted edit changes the row vs. the committed value (§12).
    pub display_rows: Vec<(InspectorRow, bool)>,
    /// Display names of dependents a staged change will also change (§7).
    pub impact_names: Vec<String>,
    /// Default parent for a new style — the current committed style, else empty.
    pub new_style_parent: String,
    /// The linked character style's `(display name, rows)`, when this paragraph
    /// style links one and it exists (§9 linked family). Read-only.
    pub linked: Option<(String, Vec<InspectorRow>)>,
}

pub(super) fn inspector_data(
    doc_state: &Arc<Mutex<DocumentState>>,
    draft: &StyleDraft,
) -> InspectorData {
    let sid = StyleId::new(&draft.id);
    let committed_rows = catalog_snapshot(doc_state)
        .map(|cat| paragraph_inspector_rows(&cat, &sid))
        .unwrap_or_default();
    let display_rows: Vec<(InspectorRow, bool)> = catalog_snapshot(doc_state)
        .map(|mut cat| {
            cat.paragraph_styles
                .insert(sid.clone(), draft_to_style(draft));
            paragraph_inspector_rows(&cat, &sid)
        })
        .unwrap_or_default()
        .into_iter()
        .map(|pending| {
            let staged = committed_rows
                .iter()
                .find(|c| c.property == pending.property)
                != Some(&pending);
            (pending, staged)
        })
        .collect();

    let changed: Vec<_> = display_rows
        .iter()
        .filter(|(_, staged)| *staged)
        .map(|(row, _)| row.property)
        .collect();
    let impact_names: Vec<String> = catalog_snapshot(doc_state)
        .map(|cat| {
            affected_dependents(&cat, &sid, &changed)
                .into_iter()
                .map(|d| display_name_of(&cat, &d))
                .collect()
        })
        .unwrap_or_default();

    let new_style_parent = if committed_rows.is_empty() {
        String::new()
    } else {
        draft.id.clone()
    };

    let linked = catalog_snapshot(doc_state).and_then(|cat| {
        let lcs = cat.paragraph_styles.get(&sid)?.linked_char_style.clone()?;
        let rows = character_inspector_rows(&cat, &lcs);
        let name = cat
            .character_styles
            .get(&lcs)
            .and_then(|s| s.display_name.clone())
            .unwrap_or_else(|| lcs.as_str().to_string());
        (!rows.is_empty()).then_some((name, rows))
    });

    InspectorData {
        display_rows,
        impact_names,
        new_style_parent,
        linked,
    }
}

fn display_name_of(cat: &loki_doc_model::style::StyleCatalog, id: &StyleId) -> String {
    cat.paragraph_styles
        .get(id)
        .and_then(|s| s.display_name.clone())
        .unwrap_or_else(|| id.as_str().to_string())
}

/// A character style's `(id, display name)` pair for the family list.
pub(super) type CharListEntry = (String, String);
/// The selected character style's `(display name, rows)` for the inspector.
pub(super) type CharSelection = Option<(String, Vec<InspectorRow>)>;

/// The character-styles browser data (§9 character family): the `(id, display)`
/// list sorted by display name, and — when `selected` names one — its
/// `(display, rows)` for the read-only inspector.
pub(super) fn char_data(
    doc_state: &Arc<Mutex<DocumentState>>,
    selected: Option<&str>,
) -> (Vec<CharListEntry>, CharSelection) {
    let Some(catalog) = catalog_snapshot(doc_state) else {
        return (Vec::new(), None);
    };
    let mut list: Vec<(String, String)> = catalog
        .character_styles
        .iter()
        // Hide synthetic internal styles (e.g. `__DocDefaultChar`, the docDefaults
        // `Default` source) — they resolve provenance, they are not user-selectable.
        .filter(|(id, _)| !id.as_str().starts_with("__"))
        .map(|(id, s)| {
            let display = s
                .display_name
                .clone()
                .unwrap_or_else(|| id.as_str().to_string());
            (id.as_str().to_string(), display)
        })
        .collect();
    list.sort_by(|(_, a), (_, b)| a.cmp(b));

    let selected_rows = selected.and_then(|sel| {
        let sid = StyleId::new(sel);
        let rows = character_inspector_rows(&catalog, &sid);
        let name = catalog
            .character_styles
            .get(&sid)
            .and_then(|s| s.display_name.clone())
            .unwrap_or_else(|| sel.to_string());
        (!rows.is_empty()).then_some((name, rows))
    });
    (list, selected_rows)
}

/// The selected list style's `(display name, per-level rows)` for the inspector.
pub(super) type ListSelection = Option<(String, Vec<ListLevelRow>)>;

/// The list-styles browser data (§9 list family; non-inheriting): the
/// `(id, display)` list sorted by display name, and — when `selected` names one
/// — its flattened per-level rows for the read-only inspector.
pub(super) fn list_data(
    doc_state: &Arc<Mutex<DocumentState>>,
    selected: Option<&str>,
) -> (Vec<CharListEntry>, ListSelection) {
    let Some(catalog) = catalog_snapshot(doc_state) else {
        return (Vec::new(), None);
    };
    let mut list: Vec<CharListEntry> = catalog
        .list_styles
        .iter()
        .map(|(id, s)| {
            let display = s
                .display_name
                .clone()
                .unwrap_or_else(|| id.as_str().to_string());
            (id.as_str().to_string(), display)
        })
        .collect();
    list.sort_by(|(_, a), (_, b)| a.cmp(b));

    let selected_rows = selected.and_then(|sel| {
        let lid = loki_doc_model::style::ListId::new(sel);
        let rows = list_inspector_rows(&catalog, &lid);
        let name = catalog
            .list_styles
            .get(&lid)
            .and_then(|s| s.display_name.clone())
            .unwrap_or_else(|| sel.to_string());
        (!rows.is_empty()).then_some((name, rows))
    });
    (list, selected_rows)
}

/// The selected page style's `(display name, geometry rows)` for the inspector.
pub(super) type PageSelection = Option<(String, Vec<PagePropRow>)>;

/// The page-styles browser data (§9 page family; non-inheriting, ADR-0012
/// Decision 2). Page styles are **derived on demand** from the live document's
/// sections (`derive_page_styles`) rather than stored — the section layouts are
/// the source of truth (the Layout ribbon mutates them directly), so deriving
/// each render keeps the panel from drifting. Returns the `(id, display)` list
/// and, when `selected` names one, its geometry rows for the read-only inspector.
pub(super) fn page_data(
    doc_state: &Arc<Mutex<DocumentState>>,
    selected: Option<&str>,
) -> (Vec<CharListEntry>, PageSelection) {
    let Ok(state) = doc_state.lock() else {
        return (Vec::new(), None);
    };
    let Some(doc) = state.document.as_ref() else {
        return (Vec::new(), None);
    };
    let styles = derive_page_styles(&doc.sections);

    let list: Vec<CharListEntry> = styles
        .iter()
        .map(|(id, ps)| {
            let display = ps
                .display_name
                .clone()
                .unwrap_or_else(|| id.as_str().to_string());
            (id.as_str().to_string(), display)
        })
        .collect();

    let selected_rows = selected.and_then(|sel| {
        let ps = styles.get(&StyleId::new(sel))?;
        let rows = page_inspector_rows(&ps.layout);
        let name = ps.display_name.clone().unwrap_or_else(|| sel.to_string());
        Some((name, rows))
    });
    (list, selected_rows)
}
