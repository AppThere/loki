// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! List membership mutations (usage audit §10 tier 1).
//!
//! A paragraph participates in a list through two direct para-props:
//! `list_id` (which [`crate::style::list_style::ListStyle`] labels it) and
//! `list_level` (0-based indent depth, 0..=8). These mutations write both;
//! the existing `clear_block_list` (in `style.rs`) removes them.
//!
//! Follows the `align.rs` shape: a plain `para` is upgraded to `styled_para`
//! first (a plain para drops its props slot on read), and each mutation has a
//! path-aware `_at` variant so lists work inside table cells and note bodies.
//! **Headings are a documented no-op**: the heading read path resolves its
//! style from the catalog and does not consult `para_props` for list
//! membership, so writing the props there would store state nothing reads
//! (numbered headings are a future feature, not a partial one).

use loro::{LoroDoc, LoroMap};

use super::{BlockPath, MutationError, get_block_map_and_list};
use crate::loro_schema::{
    BLOCK_TYPE_HEADING, BLOCK_TYPE_PARA, BLOCK_TYPE_STYLED_PARA, KEY_PARA_PROPS, KEY_TYPE,
    PROP_LIST_ID, PROP_LIST_LEVEL,
};

/// Deepest list level the model defines (levels are 0-indexed, 9 levels).
pub const MAX_LIST_LEVEL: u8 = 8;

/// The block's `KEY_TYPE` string (empty when absent).
fn block_type(block_map: &LoroMap) -> String {
    block_map
        .get(KEY_TYPE)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// The block's `para_props` sub-map, created if absent.
fn props_map_or_create(block_map: &LoroMap) -> Result<LoroMap, MutationError> {
    if let Some(existing) = block_map
        .get(KEY_PARA_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
    {
        return Ok(existing);
    }
    Ok(block_map.insert_container(KEY_PARA_PROPS, LoroMap::new())?)
}

/// Writes list membership into `block_map`, honouring its block type.
fn write_list(block_map: &LoroMap, list_id: &str, level: u8) -> Result<(), MutationError> {
    match block_type(block_map).as_str() {
        // See the module docs: nothing reads para-props list membership on a
        // heading, so writing it would be state without a reader.
        BLOCK_TYPE_HEADING => Ok(()),
        ty => {
            // A plain para drops props on read; upgrade so the list survives.
            if matches!(ty, BLOCK_TYPE_PARA | "") {
                block_map.insert(KEY_TYPE, BLOCK_TYPE_STYLED_PARA)?;
            }
            let props = props_map_or_create(block_map)?;
            props.insert(PROP_LIST_ID, list_id)?;
            props.insert(PROP_LIST_LEVEL, i32::from(level.min(MAX_LIST_LEVEL)))?;
            Ok(())
        }
    }
}

/// Reads the block's list level (0 when absent — level 0 is the outermost).
fn read_list_level(block_map: &LoroMap) -> u8 {
    block_map
        .get(KEY_PARA_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .and_then(|props| props.get(PROP_LIST_LEVEL))
        .and_then(|v| v.into_value().ok())
        .and_then(|v| match v {
            loro::LoroValue::I64(n) => u8::try_from(n).ok(),
            _ => None,
        })
        .map_or(0, |n| n.min(MAX_LIST_LEVEL))
}

/// Writes only the level, and only when the block is already a list item —
/// a `list_level` without a `list_id` is state nothing reads.
fn write_list_level(block_map: &LoroMap, level: u8) -> Result<bool, MutationError> {
    let Some(props) = block_map
        .get(KEY_PARA_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
    else {
        return Ok(false);
    };
    if props.get(PROP_LIST_ID).is_none() {
        return Ok(false);
    }
    props.insert(PROP_LIST_LEVEL, i32::from(level.min(MAX_LIST_LEVEL)))?;
    Ok(true)
}

/// Makes the top-level block at `block_index` an item of list `list_id` at
/// `level` (clamped to [`MAX_LIST_LEVEL`]). A plain paragraph is upgraded to a
/// styled paragraph; headings are a no-op (module docs).
///
/// # Errors
///
/// - [`MutationError::BlockIndexOutOfRange`] if `block_index` is out of range.
/// - [`MutationError::Loro`] for underlying Loro errors.
pub fn set_block_list(
    loro: &LoroDoc,
    block_index: usize,
    list_id: &str,
    level: u8,
) -> Result<(), MutationError> {
    let (_, block_map, _) = get_block_map_and_list(loro, block_index)?;
    write_list(&block_map, list_id, level)
}

/// Path-aware [`set_block_list`], so lists work inside table cells and note
/// bodies.
///
/// # Errors
///
/// - [`MutationError::InvalidBlockPath`] if `path` does not resolve.
/// - [`MutationError::Loro`] for underlying Loro errors.
pub fn set_block_list_at(
    loro: &LoroDoc,
    path: &BlockPath,
    list_id: &str,
    level: u8,
) -> Result<(), MutationError> {
    let block_map = super::nested::resolve_block_map(loro, path)?;
    write_list(&block_map, list_id, level)
}

/// Returns the list level of the top-level block at `block_index` (0 when the
/// block stores none — callers should gate on [`super::get_block_list_id`]
/// first, since 0 is also the outermost real level).
#[must_use]
pub fn get_block_list_level(loro: &LoroDoc, block_index: usize) -> u8 {
    get_block_map_and_list(loro, block_index)
        .map(|(_, m, _)| read_list_level(&m))
        .unwrap_or(0)
}

/// Sets the list level of the top-level block at `block_index`, clamped to
/// [`MAX_LIST_LEVEL`]. Returns `Ok(false)` — without writing — when the block
/// is not a list item (Tab in a plain paragraph must not invent one).
///
/// # Errors
///
/// - [`MutationError::BlockIndexOutOfRange`] if `block_index` is out of range.
/// - [`MutationError::Loro`] for underlying Loro errors.
pub fn set_block_list_level(
    loro: &LoroDoc,
    block_index: usize,
    level: u8,
) -> Result<bool, MutationError> {
    let (_, block_map, _) = get_block_map_and_list(loro, block_index)?;
    write_list_level(&block_map, level)
}

/// Path-aware [`get_block_list_level`].
#[must_use]
pub fn get_block_list_level_at(loro: &LoroDoc, path: &BlockPath) -> u8 {
    super::nested::resolve_block_map(loro, path)
        .map(|m| read_list_level(&m))
        .unwrap_or(0)
}

/// Path-aware [`super::get_block_list_id`].
#[must_use]
pub fn get_block_list_id_at(loro: &LoroDoc, path: &BlockPath) -> Option<String> {
    let block_map = super::nested::resolve_block_map(loro, path).ok()?;
    let props = block_map
        .get(KEY_PARA_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())?;
    props
        .get(PROP_LIST_ID)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())
}

/// Path-aware [`set_block_list_level`].
///
/// # Errors
///
/// - [`MutationError::InvalidBlockPath`] if `path` does not resolve.
/// - [`MutationError::Loro`] for underlying Loro errors.
pub fn set_block_list_level_at(
    loro: &LoroDoc,
    path: &BlockPath,
    level: u8,
) -> Result<bool, MutationError> {
    let block_map = super::nested::resolve_block_map(loro, path)?;
    write_list_level(&block_map, level)
}

/// Path-aware [`super::clear_block_list`]: removes `list_id` and `list_level`
/// from the paragraph addressed by `path`.
///
/// # Errors
///
/// - [`MutationError::InvalidBlockPath`] if `path` does not resolve.
/// - [`MutationError::Loro`] for underlying Loro errors.
pub fn clear_block_list_at(loro: &LoroDoc, path: &BlockPath) -> Result<(), MutationError> {
    let block_map = super::nested::resolve_block_map(loro, path)?;
    let Some(props) = block_map
        .get(KEY_PARA_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
    else {
        return Ok(());
    };
    if props.get(PROP_LIST_ID).is_some() {
        props.delete(PROP_LIST_ID)?;
    }
    if props.get(PROP_LIST_LEVEL).is_some() {
        props.delete(PROP_LIST_LEVEL)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
