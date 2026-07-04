// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Editing-data emission for the flow engine.
//!
//! [`push_editing_para`] records one [`PageParagraphData`] per laid-out
//! paragraph (consumed by hit-testing / cursor positioning when
//! `preserve_for_editing` is on). [`NestedEditing`] is the path context for
//! content flowed *inside* a container (currently a footnote/endnote body): while
//! set on the [`FlowState`], emitted paragraphs are addressed by the container's
//! root block + a `PathStep` descent instead of their flat block index, so the
//! editor can reach them via a `loki_doc_model::BlockPath`.

use std::sync::Arc;

use loki_doc_model::PathStep;

use super::FlowState;
use crate::para::ParagraphLayout;
use crate::result::PageParagraphData;

/// Editing-path context for nested content (see the module docs).
pub(crate) struct NestedEditing {
    pub(super) root_block: usize,
    pub(super) path: Vec<PathStep>,
}

impl NestedEditing {
    /// Context for the `body_block`-th block of `owner`'s `note_idx`-th note.
    pub(super) fn note(owner: usize, note_idx: usize, body_block: usize) -> Self {
        Self {
            root_block: owner,
            path: vec![PathStep::Note {
                note: note_idx,
                block: body_block,
            }],
        }
    }

    /// Context for the `body_block`-th block of the `cell`-th cell (in the
    /// bridge's flat head → bodies → foot order) of the table at `table`.
    pub(super) fn cell(table: usize, cell: usize, body_block: usize) -> Self {
        Self {
            root_block: table,
            path: vec![PathStep::Cell {
                cell,
                block: body_block,
            }],
        }
    }
}

/// Records a paragraph's editing data. When `state.nested_editing` is set (a
/// footnote body) the paragraph is tagged with that container's root block +
/// path; otherwise it is top-level (`path` empty).
pub(super) fn push_editing_para(
    state: &mut FlowState,
    block_index: usize,
    layout: Arc<ParagraphLayout>,
    origin: (f32, f32),
) {
    let (block_index, path) = match &state.nested_editing {
        Some(ctx) => (ctx.root_block, ctx.path.clone()),
        None => (block_index, Vec::new()),
    };
    state.current_paragraphs.push(PageParagraphData {
        block_index,
        path,
        layout,
        origin,
    });
}
