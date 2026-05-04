// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Bridge between Loki document model and Loro CRDT document.
//!
//! Public API: [`document_to_loro`] and [`loro_to_document`].
//! Internal logic is split across:
//! - `write` — serialization (Loki → Loro)
//! - `read`  — deserialization (Loro → Loki)
//! - `inlines` — inline content helpers shared by both directions

mod inlines;
mod read;
mod write;

use loro::{ExpandType, LoroDoc, LoroMap, LoroMovableList, StyleConfig, StyleConfigMap};
use crate::document::Document;
use crate::loro_schema::*;
use read::{reconstruct_blocks_from_list, reconstruct_page_layout};
use write::{map_blocks_to_list, map_page_layout};

/// Errors that can occur during document translation to/from Loro.
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    /// An error returned by the underlying Loro library.
    #[error("Loro error: {0}")]
    Loro(String),
    /// The block type at the given index is not supported by the bridge.
    #[error("Unsupported block at index {index}: {detail}")]
    UnsupportedBlock {
        /// The index of the unsupported block.
        index: usize,
        /// Description of the block type.
        detail: String,
    },
}

impl From<loro::LoroError> for BridgeError {
    fn from(e: loro::LoroError) -> Self {
        BridgeError::Loro(e.to_string())
    }
}

/// Converts a Loki [`Document`] snapshot into a fresh [`LoroDoc`] CRDT.
pub fn document_to_loro(doc: &Document) -> Result<LoroDoc, BridgeError> {
    let loro_doc = LoroDoc::new();

    // Register all mark keys so Loro tracks their expand behaviour.
    let mut style_config = StyleConfigMap::new();
    for key in &[
        MARK_BOLD, MARK_ITALIC, MARK_UNDERLINE, MARK_STRIKETHROUGH,
        MARK_COLOR, MARK_HIGHLIGHT_COLOR, MARK_FONT_FAMILY, MARK_FONT_SIZE_PT,
        MARK_VERTICAL_ALIGN, MARK_LINK_URL, MARK_LANGUAGE,
        MARK_LETTER_SPACING, MARK_WORD_SPACING, MARK_SCALE,
        MARK_SMALL_CAPS, MARK_ALL_CAPS, MARK_SHADOW, MARK_KERNING, MARK_OUTLINE,
    ] {
        style_config.insert(
            loro::InternalString::from(*key),
            StyleConfig { expand: ExpandType::After },
        );
    }
    loro_doc.config_text_style(style_config);

    // Metadata
    let meta_map = loro_doc.get_map(KEY_METADATA);
    if let Some(title) = &doc.meta.title {
        meta_map.insert("title", title.as_str())?;
    }

    // Sections
    let sections_list = loro_doc.get_list(KEY_SECTIONS);
    for (s_idx, section) in doc.sections.iter().enumerate() {
        let sec_map = sections_list.insert_container(s_idx, LoroMap::new())?;

        // Page layout (always present — Section.layout is not Option)
        map_page_layout(&section.layout, &sec_map)?;

        // Blocks
        let blocks_list = sec_map.insert_container(KEY_BLOCKS, LoroMovableList::new())?;
        map_blocks_to_list(&section.blocks, &blocks_list)?;
    }

    Ok(loro_doc)
}

/// Derives a [`Document`] snapshot from a [`LoroDoc`].
///
/// Re-derive after each CRDT mutation — the result is not kept in sync
/// automatically.
pub fn loro_to_document(loro: &LoroDoc) -> Result<Document, BridgeError> {
    let mut doc = Document::new();

    // Metadata
    let meta_map: loro::LoroMap = loro.get_map(KEY_METADATA);
    if let Some(title) = meta_map
        .get("title")
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
    {
        doc.meta.title = Some(title.to_string());
    }

    // Sections
    let sections_list = loro.get_list(KEY_SECTIONS);
    doc.sections.clear();

    for i in 0..sections_list.len() {
        let Some(sec_val) = sections_list.get(i) else { continue };
        let Some(sec_map) = sec_val.into_container().ok().and_then(|c| c.into_map().ok()) else {
            continue;
        };

        let mut section = crate::layout::section::Section::new();

        // Page layout
        section.layout = reconstruct_page_layout(&sec_map);

        // Blocks
        if let Some(blocks_val) = sec_map.get(KEY_BLOCKS)
            && let Some(blocks_list) = blocks_val
                .into_container()
                .ok()
                .and_then(|c| c.into_movable_list().ok())
        {
            section.blocks = reconstruct_blocks_from_list(&blocks_list);
        }

        doc.sections.push(section);
    }

    if doc.sections.is_empty() {
        doc.sections.push(crate::layout::section::Section::new());
    }

    Ok(doc)
}

/// Derives a stable Loro [`Cursor`] anchored to `byte_offset` within the text
/// of block `block_index` in section 0.
///
/// The cursor survives concurrent remote edits, making it suitable for
/// collaborative editing. Returns `None` when the block or text container
/// cannot be found.
///
/// [`Cursor`]: loro::cursor::Cursor
pub fn derive_loro_cursor(
    loro: &LoroDoc,
    block_index: usize,
    byte_offset: usize,
) -> Option<loro::cursor::Cursor> {
    let sections_list = loro.get_list(KEY_SECTIONS);
    let sec_map = sections_list
        .get(0)?
        .into_container()
        .ok()
        .and_then(|c| c.into_map().ok())?;

    let blocks_list = sec_map
        .get(KEY_BLOCKS)?
        .into_container()
        .ok()
        .and_then(|c| c.into_movable_list().ok())?;

    if block_index >= blocks_list.len() {
        return None;
    }

    let text = blocks_list
        .get(block_index)?
        .into_container()
        .ok()
        .and_then(|c| c.into_map().ok())?
        .get(KEY_CONTENT)?
        .into_container()
        .ok()
        .and_then(|c| c.into_text().ok())?;

    text.get_cursor(byte_offset, loro::cursor::Side::Right)
}
