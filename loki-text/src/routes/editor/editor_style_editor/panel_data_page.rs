// SPDX-License-Identifier: Apache-2.0

//! Pre-render data for the **page** family (§9; ADR-0012 Decision 2), split out
//! of [`super::panel_data`] to keep that file under the 300-line ceiling.
//!
//! # Where a page style's geometry is read from
//!
//! An **applied** page style's geometry comes from the first section that
//! references it — the renderer's own source, so the panel cannot disagree with
//! what is on screen even when the Layout ribbon edited the sections behind its
//! back. An **unapplied** one has no section, so its catalog entry is the only
//! copy there is. `set_page_style_geometry` writes both, so for a style the
//! panel itself edited the two answers are the same one.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use loki_doc_model::document::Document;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::style::StyleId;

use super::super::style_page_inspector::{PagePropRow, page_inspector_rows};
use crate::editing::state::DocumentState;

/// The selected page style's `(display name, geometry rows)` for the inspector.
pub(super) type PageSelection = Option<(String, Vec<PagePropRow>)>;

/// One page style as the panel sees it: id, the geometry to show, and the
/// sections referencing it (empty for a style created but not yet applied).
pub(super) struct PanelPageStyle {
    pub id: StyleId,
    pub layout: PageLayout,
    pub sections: Vec<usize>,
}

/// Every page style the panel lists, in catalog order.
///
/// Catalog order — not section order — because a style no section references
/// must still appear: `create_page_style` produces exactly that, and a style
/// the panel refuses to list cannot be applied, renamed or edited. A reference
/// the catalog lacks is appended after, so a section is never orphaned by a
/// catalog the importer left incomplete.
pub(super) fn panel_page_styles(doc: &Document) -> Vec<PanelPageStyle> {
    let mut refs: HashMap<&StyleId, Vec<usize>> = HashMap::new();
    for (i, section) in doc.sections.iter().enumerate() {
        if let Some(id) = section.page_style.as_ref() {
            refs.entry(id).or_default().push(i);
        }
    }
    let layout_for = |id: &StyleId, sections: &[usize]| -> PageLayout {
        sections
            .first()
            .and_then(|&i| doc.sections.get(i))
            .map(|s| s.layout.clone())
            .or_else(|| doc.styles.page_styles.get(id).map(|ps| ps.layout.clone()))
            .unwrap_or_default()
    };

    let mut out: Vec<PanelPageStyle> = doc
        .styles
        .page_styles
        .keys()
        .map(|id| {
            let sections = refs.get(id).cloned().unwrap_or_default();
            PanelPageStyle {
                id: id.clone(),
                layout: layout_for(id, &sections),
                sections,
            }
        })
        .collect();

    // Referenced but uncatalogued — keep them reachable rather than silently
    // dropping the only handle on those sections.
    for (i, section) in doc.sections.iter().enumerate() {
        let Some(id) = section.page_style.as_ref() else {
            continue;
        };
        if doc.styles.page_styles.contains_key(id) || out.iter().any(|p| p.id == *id) {
            continue;
        }
        out.push(PanelPageStyle {
            id: id.clone(),
            layout: section.layout.clone(),
            sections: refs.get(id).cloned().unwrap_or_else(|| vec![i]),
        });
    }
    out
}

/// A page style's display name: its catalogued `display_name`, else its id.
fn page_display_name(doc: &Document, id: &StyleId) -> String {
    doc.styles
        .page_styles
        .get(id)
        .and_then(|ps| ps.display_name.clone())
        .unwrap_or_else(|| id.as_str().to_string())
}

/// One browser row: `(id, display name, is applied to at least one section)`.
pub(super) type PageListEntry = (String, String, bool);

/// The page-styles browser data: every catalogued style with its display name
/// and whether any section uses it, plus — when `selected` names one — its
/// geometry rows for the inspector.
pub(super) fn page_data(
    doc_state: &Arc<Mutex<DocumentState>>,
    selected: Option<&str>,
) -> (Vec<PageListEntry>, PageSelection) {
    let Ok(state) = doc_state.lock() else {
        return (Vec::new(), None);
    };
    let Some(doc) = state.document.as_ref() else {
        return (Vec::new(), None);
    };
    let styles = panel_page_styles(doc);
    let list: Vec<PageListEntry> = styles
        .iter()
        .map(|p| {
            (
                p.id.as_str().to_string(),
                page_display_name(doc, &p.id),
                !p.sections.is_empty(),
            )
        })
        .collect();

    let selected_rows = selected.and_then(|sel| {
        let p = styles.iter().find(|p| p.id.as_str() == sel)?;
        Some((
            page_display_name(doc, &p.id),
            page_inspector_rows(&p.layout),
        ))
    });
    (list, selected_rows)
}

/// The current geometry for the page style `name` — what the edit form needs
/// before computing a preset. `None` when the document or the style is absent.
pub(super) fn page_edit_target(
    doc_state: &Arc<Mutex<DocumentState>>,
    name: &str,
) -> Option<PageLayout> {
    let state = doc_state.lock().ok()?;
    let doc = state.document.as_ref()?;
    panel_page_styles(doc)
        .into_iter()
        .find(|p| p.id.as_str() == name)
        .map(|p| p.layout)
}

/// The first page-style name not already taken, in `PageStyleN` form — the same
/// scheme `Document::assign_page_styles` uses, so a created style is named the
/// way an imported one would have been.
pub(super) fn next_page_style_name(doc_state: &Arc<Mutex<DocumentState>>) -> Option<String> {
    let state = doc_state.lock().ok()?;
    let doc = state.document.as_ref()?;
    let mut n = doc.styles.page_styles.len() + 1;
    loop {
        let cand = StyleId::new(format!("PageStyle{n}"));
        if !doc.styles.page_styles.contains_key(&cand) {
            return Some(cand.as_str().to_string());
        }
        n += 1;
    }
}

/// The index of the section containing the caret's block, for "apply this page
/// style here".
///
/// `DocumentPosition::paragraph_index` is the **flat** block index across all
/// sections (`DocumentPosition::top_level`'s second field), which is what
/// `flat_index_to_section_block` converts. `None` before the first click, so
/// the caller can withhold the control rather than silently target section 0.
pub(super) fn caret_section_index(
    doc_state: &Arc<Mutex<DocumentState>>,
    focus: Option<&crate::editing::cursor::DocumentPosition>,
) -> Option<usize> {
    let flat = focus?.paragraph_index;
    let state = doc_state.lock().ok()?;
    let doc = state.document.as_ref()?;
    doc.flat_index_to_section_block(flat).map(|(s, _)| s)
}

#[cfg(test)]
#[path = "panel_data_page_tests.rs"]
mod tests;
