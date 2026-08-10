// SPDX-License-Identifier: Apache-2.0

//! List editing operations behind the ribbon's Lists group and the Tab arm
//! (usage audit §10 tier 3): toggle the caret paragraph in/out of the default
//! bullet or numbered list, and promote/demote its level.
//!
//! All operations are path-aware, so they work inside table cells and note
//! bodies. The style *catalog* is read from the derived document (cheap Arc)
//! for button state, but written through the Loro bridge when a default style
//! has to be seeded — Loro is the styles' source of truth, and the following
//! relayout re-derives the document from it.

use std::sync::{Arc, Mutex};

use loki_doc_model::loro_mutation::{
    MAX_LIST_LEVEL, clear_block_list_at, get_block_list_id_at, get_block_list_level_at,
    set_block_list_at, set_block_list_level_at,
};
use loki_doc_model::style::list_defaults::{ensure_default_bullet, ensure_default_numbered};
use loki_doc_model::style::list_style::ListLevelKind;
use loro::LoroDoc;

use crate::editing::cursor::CursorState;
use crate::editing::state::DocumentState;

/// Which of the two default lists a ribbon button speaks for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ListKind {
    Bullet,
    Numbered,
}

/// The kind of the list `list_id` names, from its level-0 definition. `None`
/// when the catalog has no such style (an import whose definitions were lost).
fn kind_of(catalog: &loki_doc_model::StyleCatalog, list_id: &str) -> Option<ListKind> {
    let style = catalog
        .list_styles
        .get(&loki_doc_model::style::list_style::ListId::new(list_id))?;
    let level0 = style.levels.first()?;
    Some(match level0.kind {
        ListLevelKind::Bullet { .. } => ListKind::Bullet,
        _ => ListKind::Numbered,
    })
}

/// `(bullet_active, numbered_active)` for the caret paragraph — drives the
/// two toggle buttons' pressed state. Uses the derived document's catalog
/// (no Loro JSON parse per render).
pub(super) fn caret_list_state(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro: &LoroDoc,
    cursor: &CursorState,
) -> (bool, bool) {
    let Some(focus) = cursor.focus.as_ref() else {
        return (false, false);
    };
    let Some(list_id) = get_block_list_id_at(loro, &focus.block_path()) else {
        return (false, false);
    };
    let Ok(state) = doc_state.lock() else {
        return (false, false);
    };
    let Some(doc) = state.document.as_ref() else {
        return (false, false);
    };
    match kind_of(&doc.styles, &list_id) {
        Some(ListKind::Bullet) => (true, false),
        Some(ListKind::Numbered) => (false, true),
        // Membership in a lost definition still reads as "in a list" on the
        // bullet button (the export fallback treats it as a bullet list too).
        None => (true, false),
    }
}

/// Toggles the caret paragraph's membership of the default `kind` list:
/// already that kind → leave the list; another kind (or none) → join the
/// default list of `kind`, keeping the current level. Returns `true` when the
/// document changed (the caller relayouts).
pub(super) fn toggle_list(loro: &LoroDoc, cursor: &CursorState, kind: ListKind) -> bool {
    let Some(focus) = cursor.focus.as_ref() else {
        return false;
    };
    let path = focus.block_path();
    let current = get_block_list_id_at(loro, &path);

    // The catalog for both the kind check and the default-style seed comes
    // from Loro — the source of truth the relayout re-derives from.
    let mut catalog = loki_doc_model::loro_bridge::read_document_styles(loro);
    if current
        .as_deref()
        .is_some_and(|id| kind_of(&catalog, id) == Some(kind))
    {
        return clear_block_list_at(loro, &path).is_ok();
    }

    let id = match kind {
        ListKind::Bullet => ensure_default_bullet(&mut catalog),
        ListKind::Numbered => ensure_default_numbered(&mut catalog),
    };
    if loki_doc_model::loro_bridge::write_document_styles(loro, &catalog).is_err() {
        return false;
    }
    let level = get_block_list_level_at(loro, &path);
    set_block_list_at(loro, &path, id.as_str(), level).is_ok()
}

/// Promotes (`+1`) or demotes (`-1`) the caret list item's level, clamped to
/// `0..=MAX_LIST_LEVEL`. Returns `false` — changing nothing — when the caret
/// is not in a list item or already at the boundary, so Tab can fall through
/// to its default action.
pub(super) fn change_list_level(loro: &LoroDoc, cursor: &CursorState, delta: i8) -> bool {
    let Some(focus) = cursor.focus.as_ref() else {
        return false;
    };
    let path = focus.block_path();
    if get_block_list_id_at(loro, &path).is_none() {
        return false;
    }
    let current = get_block_list_level_at(loro, &path);
    let new = current.saturating_add_signed(delta).min(MAX_LIST_LEVEL);
    if new == current {
        return false;
    }
    set_block_list_level_at(loro, &path, new).unwrap_or(false)
}

#[cfg(test)]
#[path = "editor_lists_tests.rs"]
mod tests;
