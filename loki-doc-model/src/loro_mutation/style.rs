// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Block-level style mutations: read and apply named paragraph styles.

use loro::{LoroDoc, LoroMap};

use super::{MutationError, get_block_map_and_list};
use crate::loro_schema::{
    BLOCK_TYPE_HEADING, BLOCK_TYPE_PARA, BLOCK_TYPE_STYLED_PARA, KEY_HEADING_LEVEL, KEY_PARA_PROPS,
    KEY_TYPE, PROP_ALIGNMENT,
};

/// Returns a display string for the current named style of the block at
/// `block_index` in section 0.
///
/// Resolution order:
/// 1. `styled_para` block → `style_id` value (e.g. `"Normal"`, `"Body Text"`).
/// 2. `heading` block → `"Heading N"` where N is the level integer.
/// 3. `para` block (unstyled) → `"Default Paragraph Style"`.
/// 4. Any other block type → the raw type string.
///
/// Returns an empty string when `block_index` is out of range or the block
/// cannot be read, so callers can treat `""` as "no cursor / no block."
pub fn get_block_style_name(loro: &LoroDoc, block_index: usize) -> String {
    let Ok((_, block_map)) = get_block_map_and_list(loro, block_index) else {
        return String::new();
    };

    let block_type = block_map
        .get(KEY_TYPE)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())
        .unwrap_or_default();

    match block_type.as_str() {
        BLOCK_TYPE_STYLED_PARA => block_map
            .get("style_id")
            .and_then(|v| v.into_value().ok())
            .and_then(|v| v.into_string().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Default Paragraph Style".into()),

        BLOCK_TYPE_HEADING => {
            let level = block_map
                .get(KEY_HEADING_LEVEL)
                .and_then(|v| v.into_value().ok())
                .and_then(|v| {
                    if v.is_i64() {
                        v.into_i64().ok().map(|i| i as u8)
                    } else if v.is_double() {
                        v.into_double().ok().map(|d| d as u8)
                    } else {
                        None
                    }
                })
                .unwrap_or(1);
            format!("Heading {level}")
        }

        BLOCK_TYPE_PARA | "" => "Default Paragraph Style".into(),

        other => other.to_string(),
    }
}

/// Changes the block at `block_index` to an unstyled plain paragraph (`para`).
///
/// The block type is set to `BLOCK_TYPE_PARA`.  Any previously-stored
/// `style_id` or heading keys are ignored on the next bridge read because
/// `map_loro_block` dispatches exclusively on `KEY_TYPE`.
///
/// # Errors
///
/// - [`MutationError::BlockIndexOutOfRange`] if `block_index` is out of range.
/// - [`MutationError::Loro`] for underlying Loro errors.
pub fn set_block_type_para(loro: &LoroDoc, block_index: usize) -> Result<(), MutationError> {
    let (_, block_map) = get_block_map_and_list(loro, block_index)?;
    block_map.insert(KEY_TYPE, BLOCK_TYPE_PARA)?;
    Ok(())
}

/// Changes the block at `block_index` to a heading block of the given `level`.
///
/// Sets `KEY_TYPE = BLOCK_TYPE_HEADING` and `KEY_HEADING_LEVEL = level`.
/// Any previously-stored `style_id` keys are ignored on the next bridge read.
///
/// # Errors
///
/// - [`MutationError::BlockIndexOutOfRange`] if `block_index` is out of range.
/// - [`MutationError::Loro`] for underlying Loro errors.
pub fn set_block_type_heading(
    loro: &LoroDoc,
    block_index: usize,
    level: u8,
) -> Result<(), MutationError> {
    let (_, block_map) = get_block_map_and_list(loro, block_index)?;
    block_map.insert(KEY_TYPE, BLOCK_TYPE_HEADING)?;
    block_map.insert(KEY_HEADING_LEVEL, level as i64)?;
    Ok(())
}

/// Applies the named paragraph style `style_id` to the block at `block_index`.
///
/// For `styled_para` and `para` blocks, writes the `style_id` key and
/// converts the block type to `styled_para` if it was `para`.
///
/// For `heading` blocks, the block type is left as `heading`; the style name
/// is stored in the `heading_style` key (matching the OOXML mapper convention)
/// rather than replacing the heading's intrinsic level.
///
/// # Errors
///
/// - [`MutationError::BlockIndexOutOfRange`] if `block_index` is out of range.
/// - [`MutationError::Loro`] for underlying Loro errors.
pub fn set_block_style(
    loro: &LoroDoc,
    block_index: usize,
    style_id: &str,
) -> Result<(), MutationError> {
    let (_, block_map) = get_block_map_and_list(loro, block_index)?;

    let block_type = block_map
        .get(KEY_TYPE)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())
        .unwrap_or_default();

    match block_type.as_str() {
        BLOCK_TYPE_PARA | "" => {
            // Upgrade plain para to styled_para so the style_id is honoured
            // by the bridge read path.
            block_map.insert(KEY_TYPE, BLOCK_TYPE_STYLED_PARA)?;
            block_map.insert("style_id", style_id)?;
        }
        BLOCK_TYPE_STYLED_PARA => {
            block_map.insert("style_id", style_id)?;
        }
        BLOCK_TYPE_HEADING => {
            // Preserve heading type; store style name in the heading_style slot.
            block_map.insert(crate::loro_schema::KEY_HEADING_STYLE, style_id)?;
        }
        _ => {
            block_map.insert("style_id", style_id)?;
        }
    }

    Ok(())
}

/// Returns the current paragraph alignment for the block at `block_index`.
///
/// Returns `"Left"` if no alignment is stored (the default).
pub fn get_block_alignment(loro: &LoroDoc, block_index: usize) -> String {
    let Ok((_, block_map)) = get_block_map_and_list(loro, block_index) else {
        return "Left".to_string();
    };
    let Some(props_map) = block_map
        .get(KEY_PARA_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
    else {
        return "Left".to_string();
    };
    props_map
        .get(PROP_ALIGNMENT)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Left".to_string())
}

/// Sets the paragraph alignment for the block at `block_index`.
///
/// Valid values: `"Left"`, `"Center"`, `"Right"`, `"Justify"`.
/// Creates the `para_props` sub-map if it does not yet exist.
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
    let (_, block_map) = get_block_map_and_list(loro, block_index)?;
    let props_map = if let Some(existing) = block_map
        .get(KEY_PARA_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
    {
        existing
    } else {
        block_map.insert_container(KEY_PARA_PROPS, LoroMap::new())?
    };
    props_map.insert(PROP_ALIGNMENT, alignment)?;
    Ok(())
}
