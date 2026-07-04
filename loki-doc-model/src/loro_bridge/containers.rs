// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Native CRDT mappings for container blocks: bullet/ordered lists, block
//! quotes, divs, and figures.
//!
//! Follows the `table.rs` pattern: structural metadata that rarely changes is
//! stored as a small `serde`-JSON snapshot under [`KEY_CONTAINER_META`], while
//! the *content* — list items, quote/div children, figure caption and body —
//! lives in nested movable lists written through the shared block path
//! ([`super::write::map_blocks_to_list`]). Paragraph text inside a list item
//! therefore sits in real `LoroText` containers, so concurrent edits to
//! different items merge instead of conflicting on one opaque JSON blob.
//!
//! Layout per block type:
//! - `BulletList`   → [`KEY_LIST_ITEMS`] (one nested block list per item)
//! - `OrderedList`  → [`KEY_CONTAINER_META`] = `ListAttributes` JSON +
//!   [`KEY_LIST_ITEMS`]
//! - `BlockQuote`   → [`KEY_CHILD_BLOCKS`]
//! - `Div`          → [`KEY_CONTAINER_META`] = `NodeAttr` JSON +
//!   [`KEY_CHILD_BLOCKS`]
//! - `Figure`       → [`KEY_CONTAINER_META`] = `(NodeAttr, short caption)`
//!   JSON + [`KEY_CAPTION_BLOCKS`] + [`KEY_CHILD_BLOCKS`]
//!
//! On read, a block-type tag with no content lists is a legacy stub written
//! by a bridge version that predates both this mapping and the opaque
//! snapshot scheme; it carries nothing recoverable and falls back to
//! [`Block::HorizontalRule`], matching `table.rs`. Without the `serde`
//! feature there is no metadata format, so these blocks keep taking the
//! opaque path on write.

#[cfg(feature = "serde")]
use super::BridgeError;
use crate::content::block::Block;
use crate::loro_schema::{
    BLOCK_TYPE_BLOCKQUOTE, BLOCK_TYPE_BULLET_LIST, BLOCK_TYPE_DIV, BLOCK_TYPE_FIGURE,
    BLOCK_TYPE_ORDERED_LIST, KEY_CAPTION_BLOCKS, KEY_CHILD_BLOCKS, KEY_LIST_ITEMS,
};
#[cfg(feature = "serde")]
use crate::loro_schema::{KEY_CONTAINER_META, KEY_TYPE};
use loro::LoroMap;
#[cfg(feature = "serde")]
use loro::LoroMovableList;

// ── Write path ────────────────────────────────────────────────────────────────

/// Writes a container block (`BulletList`/`OrderedList`/`BlockQuote`/`Div`/
/// `Figure`) into `map` as its native block type. Callers guarantee `block`
/// is one of those variants.
#[cfg(feature = "serde")]
pub(super) fn write_container(block: &Block, map: &LoroMap) -> Result<(), BridgeError> {
    match block {
        Block::BulletList(items) => {
            map.insert(KEY_TYPE, BLOCK_TYPE_BULLET_LIST)?;
            write_items(items, map)
        }
        Block::OrderedList(attrs, items) => {
            map.insert(KEY_TYPE, BLOCK_TYPE_ORDERED_LIST)?;
            write_meta(attrs, map)?;
            write_items(items, map)
        }
        Block::BlockQuote(children) => {
            map.insert(KEY_TYPE, BLOCK_TYPE_BLOCKQUOTE)?;
            write_blocks(children, map, KEY_CHILD_BLOCKS)
        }
        Block::Div(attr, children) => {
            map.insert(KEY_TYPE, BLOCK_TYPE_DIV)?;
            write_meta(attr, map)?;
            write_blocks(children, map, KEY_CHILD_BLOCKS)
        }
        Block::Figure(attr, caption, content) => {
            map.insert(KEY_TYPE, BLOCK_TYPE_FIGURE)?;
            write_meta(&(attr, &caption.short), map)?;
            write_blocks(&caption.full, map, KEY_CAPTION_BLOCKS)?;
            write_blocks(content, map, KEY_CHILD_BLOCKS)
        }
        // Guarded by the caller's dispatch; preserve rather than lose.
        other => super::opaque::write_opaque_block(other, map),
    }
}

#[cfg(feature = "serde")]
fn write_meta<T: serde::Serialize>(meta: &T, map: &LoroMap) -> Result<(), BridgeError> {
    match serde_json::to_string(meta) {
        Ok(json) => {
            map.insert(KEY_CONTAINER_META, json)?;
        }
        Err(err) => {
            // Unreachable in practice: every model type derives Serialize.
            tracing::warn!("loro bridge: failed to snapshot container meta: {err}");
        }
    }
    Ok(())
}

#[cfg(feature = "serde")]
fn write_items(items: &[Vec<Block>], map: &LoroMap) -> Result<(), BridgeError> {
    let items_list = map.insert_container(KEY_LIST_ITEMS, LoroMovableList::new())?;
    for (i, item_blocks) in items.iter().enumerate() {
        let item_list = items_list.insert_container(i, LoroMovableList::new())?;
        super::write::map_blocks_to_list(item_blocks, &item_list)?;
    }
    Ok(())
}

#[cfg(feature = "serde")]
fn write_blocks(blocks: &[Block], map: &LoroMap, key: &str) -> Result<(), BridgeError> {
    let list = map.insert_container(key, LoroMovableList::new())?;
    super::write::map_blocks_to_list(blocks, &list)
}

// ── Read path ────────────────────────────────────────────────────────────────

/// Reads a native container block back into its [`Block`] variant. `block_type`
/// is the map's [`KEY_TYPE`] tag. Falls back to [`Block::HorizontalRule`] for
/// legacy stubs (no content lists) or unknown tags.
pub(super) fn read_container(block_type: &str, map: &LoroMap) -> Block {
    let block = match block_type {
        BLOCK_TYPE_BULLET_LIST => read_items(map).map(Block::BulletList),
        BLOCK_TYPE_ORDERED_LIST => match (read_meta(map), read_items(map)) {
            (Some(attrs), Some(items)) => Some(Block::OrderedList(attrs, items)),
            _ => None,
        },
        BLOCK_TYPE_BLOCKQUOTE => read_blocks(map, KEY_CHILD_BLOCKS).map(Block::BlockQuote),
        BLOCK_TYPE_DIV => match (read_meta(map), read_blocks(map, KEY_CHILD_BLOCKS)) {
            (Some(attr), Some(children)) => Some(Block::Div(attr, children)),
            _ => None,
        },
        BLOCK_TYPE_FIGURE => read_figure(map),
        _ => None,
    };
    block.unwrap_or_else(|| {
        tracing::warn!("loro bridge: unreadable native {block_type} block; dropping to rule");
        Block::HorizontalRule
    })
}

fn read_figure(map: &LoroMap) -> Option<Block> {
    let (attr, short): (
        crate::content::attr::NodeAttr,
        Option<Vec<crate::content::inline::Inline>>,
    ) = read_meta(map)?;
    let full = read_blocks(map, KEY_CAPTION_BLOCKS)?;
    let content = read_blocks(map, KEY_CHILD_BLOCKS)?;
    Some(Block::Figure(
        attr,
        crate::content::block::Caption { short, full },
        content,
    ))
}

#[cfg(feature = "serde")]
fn read_meta<T: serde::de::DeserializeOwned>(map: &LoroMap) -> Option<T> {
    let json = map
        .get(KEY_CONTAINER_META)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())?;
    serde_json::from_str(&json).ok()
}

#[cfg(not(feature = "serde"))]
fn read_meta<T>(_map: &LoroMap) -> Option<T> {
    None
}

fn read_items(map: &LoroMap) -> Option<Vec<Vec<Block>>> {
    let items_list = map
        .get(KEY_LIST_ITEMS)?
        .into_container()
        .ok()?
        .into_movable_list()
        .ok()?;
    let mut items = Vec::with_capacity(items_list.len());
    for i in 0..items_list.len() {
        let item_list = items_list
            .get(i)?
            .into_container()
            .ok()?
            .into_movable_list()
            .ok()?;
        items.push(super::read::reconstruct_blocks_from_list(&item_list));
    }
    Some(items)
}

fn read_blocks(map: &LoroMap, key: &str) -> Option<Vec<Block>> {
    let list = map
        .get(key)?
        .into_container()
        .ok()?
        .into_movable_list()
        .ok()?;
    Some(super::read::reconstruct_blocks_from_list(&list))
}
