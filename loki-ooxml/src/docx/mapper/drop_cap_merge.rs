// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Drop-cap frame merging.
//!
//! Word encodes a dropped initial as **two** paragraphs: a framed paragraph
//! (`w:framePr w:dropCap`) containing just the enlarged initial, immediately
//! followed by the body paragraph, which wraps around the floated frame.
//!
//! Loki's layout models a drop cap as a single paragraph whose leading
//! character is enlarged (the ODF `style:drop-cap` model). This pass bridges
//! the two: it folds a drop-cap frame paragraph into the following body
//! paragraph — prepending the initial run(s) and moving the drop-cap property
//! onto the body — so the renderer's single-paragraph path applies.

use loki_doc_model::content::block::Block;
use loki_doc_model::style::props::para_props::ParaProps;

/// Merges drop-cap frame paragraphs into their following body paragraph.
///
/// A block qualifies as a frame when it is a [`Block::StyledPara`] carrying a
/// `drop_cap` in its direct paragraph properties. When the next block is also a
/// styled paragraph, the two are merged; otherwise the frame is left untouched
/// (it then renders as a normal — if large — initial).
pub(crate) fn merge_drop_cap_frames(blocks: Vec<Block>) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::with_capacity(blocks.len());
    let mut iter = blocks.into_iter().peekable();

    while let Some(block) = iter.next() {
        let frame_dc = match &block {
            Block::StyledPara(p) => p.direct_para_props.as_ref().and_then(|pp| pp.drop_cap),
            _ => None,
        };

        if let Some(dc) = frame_dc {
            // Only merge when a styled body paragraph follows the frame.
            if matches!(iter.peek(), Some(Block::StyledPara(_))) {
                let Block::StyledPara(frame) = block else {
                    unreachable!("frame_dc is Some only for StyledPara");
                };
                let Some(Block::StyledPara(mut body)) = iter.next() else {
                    unreachable!("peeked a StyledPara");
                };

                // Prepend the frame's initial run(s) to the body text.
                let mut inlines = frame.inlines;
                inlines.append(&mut body.inlines);
                body.inlines = inlines;

                // Move the drop-cap property onto the body paragraph.
                let mut pp: ParaProps = body.direct_para_props.map(|b| *b).unwrap_or_default();
                pp.drop_cap = Some(dc);
                body.direct_para_props = Some(Box::new(pp));

                out.push(Block::StyledPara(body));
                continue;
            }
        }

        out.push(block);
    }

    out
}

#[cfg(test)]
#[path = "drop_cap_merge_tests.rs"]
mod tests;
