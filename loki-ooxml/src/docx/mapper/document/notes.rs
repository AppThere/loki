// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Note (footnote/endnote) pre-processing: maps note parts to `Block` maps.

use std::collections::HashMap;

use loki_doc_model::content::block::Block;

use crate::docx::model::footnotes::{DocxNoteType, DocxNotes};

use super::super::paragraph::map_paragraph;
use super::context::MappingContext;

/// Maps a notes part to a `HashMap<id, Vec<Block>>` using the given context.
///
/// Only `Normal`-type notes are included; separators and continuation
/// separators are skipped. The context should use empty note maps to avoid
/// circular dependencies (notes referencing notes is not supported in v0.1.0).
pub(super) fn map_notes_to_blocks(
    notes: Option<&DocxNotes>,
    ctx: &mut MappingContext<'_>,
) -> HashMap<i32, Vec<Block>> {
    let Some(notes) = notes else {
        return HashMap::new();
    };
    notes
        .notes
        .iter()
        .filter(|n| n.note_type == DocxNoteType::Normal)
        .map(|n| {
            let blocks: Vec<Block> = n
                .paragraphs
                .iter()
                .flat_map(|p| map_paragraph(p, ctx))
                .collect();
            (n.id, blocks)
        })
        .collect()
}
