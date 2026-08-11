// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Catalog mutations for **list styles** (§10 tier 5 — the style panel's
//! per-level editor). List styles are flat per-level definitions with no
//! inheritance, so editing is a read-modify-write of the catalog entry; the
//! relayout that follows re-derives every referencing paragraph's markers.

use loro::LoroDoc;

use crate::MutationError;
use crate::style::list_style::{ListId, ListLevel};

/// Replaces level `level` of the list style `id` with `new_level`. A no-op
/// `Ok` when `id` is not a list style or the level does not exist — the
/// panel's form only shows real levels, so an absent target means the
/// document changed under it, which is not an error.
///
/// `new_level.level` is forced to `level` — a form cannot move a definition
/// to a different indent by mislabeling it.
///
/// # Errors
///
/// [`MutationError::Loro`] for an underlying Loro error.
pub fn set_list_style_level(
    loro: &LoroDoc,
    id: &str,
    level: u8,
    mut new_level: ListLevel,
) -> Result<(), MutationError> {
    let mut catalog = crate::loro_bridge::read_document_styles(loro);
    let Some(style) = catalog.list_styles.get_mut(&ListId::new(id)) else {
        return Ok(());
    };
    let Some(slot) = style.levels.get_mut(usize::from(level)) else {
        return Ok(());
    };
    new_level.level = level;
    *slot = new_level;
    crate::loro_bridge::write_document_styles(loro, &catalog)
        .map_err(|e| MutationError::Loro(e.to_string()))
}

#[cfg(test)]
#[path = "list_style_edit_tests.rs"]
mod tests;
