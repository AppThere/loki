// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Whole-document text-container enumeration.
//!
//! [`collect_all_text_containers`] walks every section's block list and returns
//! each block's `LoroText` content container, **descending into table cells and
//! note bodies** (recursively — a table in a cell, or a note in a cell, is
//! reached too). This is the set the Review-tab accept/reject-all sweep resolves,
//! so a tracked change anywhere in the tree — not just a top-level paragraph — is
//! honoured. Unlike [`super::nested`]'s `BlockPath` addressing (which targets one
//! known block), this collects the full set without needing paths.

use loro::{LoroDoc, LoroMap, LoroMovableList, LoroText};

use super::section_blocks_list;
use crate::loro_schema::{KEY_CONTENT, KEY_NOTES, KEY_SECTIONS, KEY_TABLE_CELLS};

/// Collects every block's `LoroText` content container across the document,
/// descending into table cells and note bodies.
pub(super) fn collect_all_text_containers(loro: &LoroDoc) -> Vec<LoroText> {
    let mut out = Vec::new();
    let sections = loro.get_list(KEY_SECTIONS);
    for s in 0..sections.len() {
        if let Some(list) = section_blocks_list(&sections, s) {
            collect_from_block_list(&list, &mut out);
        }
    }
    out
}

/// Recurses a movable list of block maps, collecting each block's text.
fn collect_from_block_list(list: &LoroMovableList, out: &mut Vec<LoroText>) {
    for i in 0..list.len() {
        if let Some(map) = list
            .get(i)
            .and_then(|v| v.into_container().ok())
            .and_then(|c| c.into_map().ok())
        {
            collect_from_block_map(&map, out);
        }
    }
}

/// Collects one block's own text, then descends into its table cells / notes.
fn collect_from_block_map(block_map: &LoroMap, out: &mut Vec<LoroText>) {
    if let Some(text) = block_map
        .get(KEY_CONTENT)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_text().ok())
    {
        out.push(text);
    }
    for key in [KEY_TABLE_CELLS, KEY_NOTES] {
        collect_nested(block_map, key, out);
    }
}

/// Descends a block's `KEY_TABLE_CELLS` / `KEY_NOTES` container — a movable list
/// of cell/note block lists — recursing into every block within.
fn collect_nested(block_map: &LoroMap, key: &str, out: &mut Vec<LoroText>) {
    let Some(container) = block_map
        .get(key)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_movable_list().ok())
    else {
        return;
    };
    for i in 0..container.len() {
        if let Some(inner) = container
            .get(i)
            .and_then(|v| v.into_container().ok())
            .and_then(|c| c.into_movable_list().ok())
        {
            collect_from_block_list(&inner, out);
        }
    }
}
