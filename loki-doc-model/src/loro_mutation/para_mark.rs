// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph-mark tracked deletion (Review tab 4a.2).
//!
//! A paragraph's terminating mark (¶) is modelled by its
//! `direct_char_props` (the OOXML `w:pPr/w:rPr` slot); a tracked deletion of that
//! mark is a `Deletion` [`RevisionMark`] stored under [`PROP_REVISION`] in the
//! block's [`KEY_DIRECT_CHAR_PROPS`] sub-map (round-tripped by the bridge). It
//! means "the ¶ is deleted": on **accept** the paragraph merges with its
//! successor, on **reject** the mark clears.
//!
//! [`set_para_mark_deletion`] records one (Backspace at a paragraph start under
//! track changes); [`resolve_para_marks`] is the accept/reject-all sweep, which
//! also descends into table cells and note bodies.

use loro::{LoroDoc, LoroMap, LoroMovableList};

use super::block::merge_block_in_list;
use super::{MutationError, get_block_map_and_list, section_blocks_list};
use crate::loro_schema::{
    BLOCK_TYPE_PARA, BLOCK_TYPE_STYLED_PARA, KEY_DIRECT_CHAR_PROPS, KEY_NOTES, KEY_SECTIONS,
    KEY_TABLE_CELLS, KEY_TYPE, PROP_REVISION,
};
use crate::style::props::revision::{RevisionKind, RevisionMark, decode, encode};

/// The `KEY_TYPE` string of a block map ("" if absent).
fn block_type(map: &LoroMap) -> String {
    map.get(KEY_TYPE)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// The block's `KEY_DIRECT_CHAR_PROPS` sub-map, creating it if absent.
fn get_or_create_char_props(map: &LoroMap) -> Result<LoroMap, MutationError> {
    if let Some(existing) = map
        .get(KEY_DIRECT_CHAR_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
    {
        Ok(existing)
    } else {
        Ok(map.insert_container(KEY_DIRECT_CHAR_PROPS, LoroMap::new())?)
    }
}

/// Records a tracked deletion of the paragraph mark on the block at
/// `block_index` — its terminating ¶ (OOXML `w:pPr/w:rPr/w:del`). A plain `para`
/// is upgraded to `styled_para` so the mark survives (as alignment does).
///
/// Returns `false` (recording nothing) when the block is not a paragraph (a
/// heading, table, …), so the caller can fall back to a hard merge.
///
/// # Errors
///
/// [`MutationError`] for an underlying index / Loro error.
pub fn set_para_mark_deletion(
    loro: &LoroDoc,
    block_index: usize,
    mark: &RevisionMark,
) -> Result<bool, MutationError> {
    let (_, map, _) = get_block_map_and_list(loro, block_index)?;
    match block_type(&map).as_str() {
        BLOCK_TYPE_PARA | "" => map.insert(KEY_TYPE, BLOCK_TYPE_STYLED_PARA)?,
        BLOCK_TYPE_STYLED_PARA => {}
        _ => return Ok(false), // heading / table / … — not a paragraph mark
    }
    get_or_create_char_props(&map)?.insert(PROP_REVISION, encode(mark))?;
    Ok(true)
}

/// The paragraph-mark revision kind of a block (`None` if its mark is untracked).
fn read_para_mark(map: &LoroMap) -> Option<RevisionKind> {
    map.get(KEY_DIRECT_CHAR_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .and_then(|props| props.get(PROP_REVISION))
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .and_then(|s| decode(&s))
        .map(|m| m.kind)
}

/// Clears a block's paragraph-mark revision (its ¶ is no longer tracked-deleted).
fn clear_para_mark(map: &LoroMap) -> Result<(), MutationError> {
    if let Some(props) = map
        .get(KEY_DIRECT_CHAR_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
    {
        props.delete(PROP_REVISION)?;
    }
    Ok(())
}

/// The block map at index `i` of `list`, if it resolves to one.
fn block_map_at(list: &LoroMovableList, i: usize) -> Option<LoroMap> {
    list.get(i)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
}

/// The nested block lists under `key` (`KEY_TABLE_CELLS` / `KEY_NOTES`) of a
/// block map — each cell / note body's block list.
fn nested_lists(map: &LoroMap, key: &str) -> Vec<LoroMovableList> {
    let Some(container) = map
        .get(key)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_movable_list().ok())
    else {
        return Vec::new();
    };
    (0..container.len())
        .filter_map(|i| {
            container
                .get(i)
                .and_then(|v| v.into_container().ok())
                .and_then(|c| c.into_movable_list().ok())
        })
        .collect()
}

/// Accepts (`accept = true`) or rejects every paragraph-mark deletion in the
/// document, returning the count resolved. On accept a struck ¶ merges its
/// paragraph with the next; on reject the mark clears. Descends into table cells
/// and note bodies.
pub(super) fn resolve_para_marks(loro: &LoroDoc, accept: bool) -> Result<usize, MutationError> {
    let mut count = 0;
    let sections = loro.get_list(KEY_SECTIONS);
    for s in 0..sections.len() {
        if let Some(list) = section_blocks_list(&sections, s) {
            count += resolve_list(&list, accept)?;
        }
    }
    Ok(count)
}

/// Resolves paragraph-mark deletions in one block list, recursing into each
/// block's nested containers first (before any merge shifts this list).
fn resolve_list(list: &LoroMovableList, accept: bool) -> Result<usize, MutationError> {
    let mut count = 0;
    for i in 0..list.len() {
        if let Some(map) = block_map_at(list, i) {
            for key in [KEY_TABLE_CELLS, KEY_NOTES] {
                for inner in nested_lists(&map, key) {
                    count += resolve_list(&inner, accept)?;
                }
            }
        }
    }
    let mut i = 0;
    while i < list.len() {
        if block_map_at(list, i).as_ref().and_then(read_para_mark) == Some(RevisionKind::Deletion) {
            count += 1;
            // On accept the successor merges into block i; a non-text successor
            // (e.g. a table) cannot merge, so the mark just clears.
            if accept && i + 1 < list.len() {
                match merge_block_in_list(list, i + 1, 0) {
                    Ok(_) | Err(MutationError::TextNotFound(_)) => {}
                    Err(e) => return Err(e),
                }
            }
            if let Some(map) = block_map_at(list, i) {
                clear_para_mark(&map)?;
            }
        }
        i += 1;
    }
    Ok(count)
}
