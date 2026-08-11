// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Section-level mutations (§3d). Until this existed, multi-section
//! documents only arose from import — every named-page-style verb had UI and
//! mutation but nothing could *create* a second section in-app.
//!
//! The UI hookup (an Insert-tab section-break action) is the deliberate
//! follow-up; this module is the §3d work item — the mutation, parked
//! reachable-through-`pub` like the tier-1 list mutations were before their
//! ribbon landed.

use loro::{LoroDoc, LoroMap, LoroMovableList};

use crate::MutationError;
use crate::content::block::Block;
use crate::loro_schema::{KEY_BLOCKS, KEY_PAGE_STYLE_REF, KEY_SECTIONS};

use super::page_style::{section_at, section_ref};

/// Inserts a new section after `section_index`, continuing its page setup:
/// the full page layout (size, orientation, margins, header/footer bands,
/// columns) and the named page-style reference are copied from the source
/// section — a fresh section reading differently from the page it follows
/// would be a surprise, and reassignment is one `set_section_page_style`
/// away. Content starts as a single empty paragraph (a section with no
/// blocks is uneditable — there is nowhere to put a caret).
///
/// Returns the new section's index.
///
/// # Errors
///
/// - [`MutationError::SectionIndexOutOfRange`] if `section_index` does not
///   name a section.
/// - [`MutationError::Loro`] / [`MutationError::Encode`] for underlying
///   errors.
pub fn insert_section_after(loro: &LoroDoc, section_index: usize) -> Result<usize, MutationError> {
    let sections = loro.get_list(KEY_SECTIONS);
    let Some(source) = section_at(&sections, section_index) else {
        return Err(MutationError::SectionIndexOutOfRange(section_index));
    };

    // The source's full layout, via the derived document — the read path the
    // whole editor trusts, rather than a second hand-rolled decoder here.
    let doc = crate::loro_bridge::loro_to_document(loro)
        .map_err(|e| MutationError::Encode(e.to_string()))?;
    let Some(layout) = doc.sections.get(section_index).map(|s| s.layout.clone()) else {
        return Err(MutationError::SectionIndexOutOfRange(section_index));
    };

    let new_map = sections.insert_container(section_index + 1, LoroMap::new())?;
    crate::loro_bridge::map_page_layout(&layout, &new_map)
        .map_err(|e| MutationError::Encode(e.to_string()))?;
    if let Some(style_ref) = section_ref(&source) {
        new_map.insert(KEY_PAGE_STYLE_REF, style_ref)?;
    }

    let blocks = new_map.insert_container(KEY_BLOCKS, LoroMovableList::new())?;
    let para_map = blocks.insert_container(0, LoroMap::new())?;
    crate::loro_bridge::map_block(&Block::Para(Vec::new()), &para_map)
        .map_err(|e| MutationError::Encode(e.to_string()))?;

    Ok(section_index + 1)
}

/// The section containing the global top-level `block_index`, mirroring
/// [`resolve_section_blocks`](super::resolve_section_blocks)'s walk — the UI
/// resolves the caret's section with this before calling
/// [`insert_section_after`]. `None` when the index is past the last block.
#[must_use]
pub fn section_of_block(loro: &LoroDoc, block_index: usize) -> Option<usize> {
    let sections = loro.get_list(KEY_SECTIONS);
    let mut base = 0usize;
    for s in 0..sections.len() {
        let Some(section) = section_at(&sections, s) else {
            continue;
        };
        let len = section
            .get(KEY_BLOCKS)
            .and_then(|v| v.into_container().ok())
            .and_then(|c| c.into_movable_list().ok())
            .map_or(0, |l| l.len());
        if block_index < base + len {
            return Some(s);
        }
        base += len;
    }
    None
}

#[cfg(test)]
#[path = "section_tests.rs"]
mod tests;
