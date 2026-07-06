// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph alignment mutations (top-level and path-aware).
//!
//! Alignment is stored differently per block type, so read/write branch on it:
//!
//! - **`para`** — a plain paragraph has no props slot on read, so it is first
//!   upgraded to **`styled_para`** (the same trick [`super::style::set_block_style`]
//!   uses), then the alignment goes in `para_props`.
//! - **`styled_para`** — alignment goes straight into `para_props`.
//! - **`heading`** — headings carry alignment as an OOXML `jc` attribute
//!   (`KEY_HEADING_JC`, lowercase `center`/`right`/`justify`), not `para_props`.
//!
//! Public values are the para-props spelling: `"Left"`, `"Center"`, `"Right"`,
//! `"Justify"` (matching `encode_alignment`/`decode_alignment`).

use loro::{LoroDoc, LoroMap};

use super::{BlockPath, MutationError, get_block_map_and_list};
use crate::loro_schema::{
    BLOCK_TYPE_HEADING, BLOCK_TYPE_PARA, BLOCK_TYPE_STYLED_PARA, KEY_HEADING_JC, KEY_PARA_PROPS,
    KEY_TYPE, PROP_ALIGNMENT,
};

/// The block's `KEY_TYPE` string (empty when absent).
fn block_type(block_map: &LoroMap) -> String {
    block_map
        .get(KEY_TYPE)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

/// Para-props alignment → the OOXML `jc` value a heading uses. `"Left"` maps to
/// `"left"`, which the layout treats as the (default) left alignment.
fn jc_from_alignment(alignment: &str) -> &'static str {
    match alignment {
        "Center" => "center",
        "Right" => "right",
        "Justify" => "justify",
        _ => "left",
    }
}

/// Inverse of [`jc_from_alignment`].
fn alignment_from_jc(jc: &str) -> &'static str {
    match jc {
        "center" => "Center",
        "right" => "Right",
        "justify" => "Justify",
        _ => "Left",
    }
}

/// Reads the alignment of `block_map` (`"Left"` default).
fn read_alignment(block_map: &LoroMap) -> String {
    if block_type(block_map) == BLOCK_TYPE_HEADING {
        return block_map
            .get(KEY_HEADING_JC)
            .and_then(|v| v.into_value().ok())
            .and_then(|v| v.into_string().ok())
            .map(|s| alignment_from_jc(&s).to_string())
            .unwrap_or_else(|| "Left".to_string());
    }
    block_map
        .get(KEY_PARA_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        .and_then(|props| props.get(PROP_ALIGNMENT))
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Left".to_string())
}

/// Writes `alignment` into a block's `para_props`, creating the sub-map if absent.
fn write_para_alignment(block_map: &LoroMap, alignment: &str) -> Result<(), MutationError> {
    let props = if let Some(existing) = block_map
        .get(KEY_PARA_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
    {
        existing
    } else {
        block_map.insert_container(KEY_PARA_PROPS, LoroMap::new())?
    };
    props.insert(PROP_ALIGNMENT, alignment)?;
    Ok(())
}

/// Writes `alignment` into `block_map`, honouring its block type.
fn write_alignment(block_map: &LoroMap, alignment: &str) -> Result<(), MutationError> {
    match block_type(block_map).as_str() {
        BLOCK_TYPE_HEADING => {
            block_map.insert(KEY_HEADING_JC, jc_from_alignment(alignment))?;
        }
        // A plain para drops props on read; upgrade so alignment survives.
        BLOCK_TYPE_PARA | "" => {
            block_map.insert(KEY_TYPE, BLOCK_TYPE_STYLED_PARA)?;
            write_para_alignment(block_map, alignment)?;
        }
        _ => write_para_alignment(block_map, alignment)?,
    }
    Ok(())
}

/// Returns the alignment of the top-level block at `block_index` (`"Left"` if
/// none is stored).
pub fn get_block_alignment(loro: &LoroDoc, block_index: usize) -> String {
    get_block_map_and_list(loro, block_index)
        .map(|(_, m, _)| read_alignment(&m))
        .unwrap_or_else(|_| "Left".to_string())
}

/// Sets the alignment of the top-level block at `block_index`.
///
/// Valid values: `"Left"`, `"Center"`, `"Right"`, `"Justify"`. A plain
/// paragraph is upgraded to a styled paragraph so the alignment persists.
///
/// # Errors
///
/// - [`MutationError::BlockIndexOutOfRange`] if `block_index` is out of range.
/// - [`MutationError::Loro`] for underlying Loro errors.
pub fn set_block_alignment(
    loro: &LoroDoc,
    block_index: usize,
    alignment: &str,
) -> Result<(), MutationError> {
    let (_, block_map, _) = get_block_map_and_list(loro, block_index)?;
    write_alignment(&block_map, alignment)
}

/// Path-aware [`get_block_alignment`]: reads the alignment of the paragraph
/// addressed by `path` (top-level, or nested in a table cell / note body).
pub fn get_block_alignment_at(loro: &LoroDoc, path: &BlockPath) -> String {
    super::nested::resolve_block_map(loro, path)
        .map(|m| read_alignment(&m))
        .unwrap_or_else(|_| "Left".to_string())
}

/// Path-aware [`set_block_alignment`]: aligns the paragraph addressed by `path`,
/// so alignment works inside table cells and note bodies.
///
/// # Errors
///
/// - [`MutationError::InvalidBlockPath`] if `path` does not resolve.
/// - [`MutationError::Loro`] for underlying Loro errors.
pub fn set_block_alignment_at(
    loro: &LoroDoc,
    path: &BlockPath,
    alignment: &str,
) -> Result<(), MutationError> {
    let block_map = super::nested::resolve_block_map(loro, path)?;
    write_alignment(&block_map, alignment)
}
