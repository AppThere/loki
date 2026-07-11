// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph-mark deletion resolution for the pure accept/reject transforms
//! (Review tab 4a.2).
//!
//! A paragraph's mark (¶) is tracked-deleted when its `direct_char_props.revision`
//! is a `Deletion` (OOXML `w:pPr/w:rPr/w:del`). On **accept** the ¶ is removed —
//! the paragraph's successor merges into it; on **reject** the mark clears and the
//! paragraphs stay split. A non-paragraph successor cannot merge, so the mark just
//! clears. This mirrors the CRDT sweep in `loro_mutation::para_mark`.

use crate::content::block::Block;
use crate::content::inline::Inline;
use crate::style::props::revision::RevisionKind;

/// Whether a paragraph carries a tracked-deletion on its mark.
fn para_mark_deletion(block: &Block) -> bool {
    matches!(block, Block::StyledPara(p)
        if p.direct_char_props.as_ref()
            .and_then(|c| c.revision.as_ref())
            .is_some_and(|m| m.kind == RevisionKind::Deletion))
}

/// Clears a paragraph's mark revision (its ¶ is no longer tracked-deleted).
fn clear_para_mark(block: &mut Block) {
    if let Block::StyledPara(p) = block
        && let Some(c) = p.direct_char_props.as_mut()
    {
        c.revision = None;
    }
}

/// The inline content of a paragraph-like block (for merging on accept); `None`
/// for a non-paragraph block (table, rule, …) that cannot be merged.
fn para_inlines(block: &Block) -> Option<Vec<Inline>> {
    match block {
        Block::Para(i) | Block::Plain(i) | Block::Heading(_, _, i) => Some(i.clone()),
        Block::StyledPara(p) => Some(p.inlines.clone()),
        _ => None,
    }
}

/// Appends `extra` inlines to a paragraph-like block's content.
fn append_inlines(block: &mut Block, mut extra: Vec<Inline>) {
    match block {
        Block::Para(i) | Block::Plain(i) | Block::Heading(_, _, i) => i.append(&mut extra),
        Block::StyledPara(p) => p.inlines.append(&mut extra),
        _ => {}
    }
}

/// Resolves paragraph-mark deletions in one block list: on `accept` a struck
/// paragraph's ¶ is removed (its successor merges into it); otherwise the mark is
/// cleared (the paragraphs stay split).
pub(super) fn resolve_para_marks(blocks: &mut Vec<Block>, accept: bool) {
    let mut i = 0;
    while i < blocks.len() {
        if para_mark_deletion(&blocks[i]) {
            if accept
                && i + 1 < blocks.len()
                && let Some(next) = para_inlines(&blocks[i + 1])
            {
                append_inlines(&mut blocks[i], next);
                blocks.remove(i + 1);
            }
            clear_para_mark(&mut blocks[i]);
        }
        i += 1;
    }
}
