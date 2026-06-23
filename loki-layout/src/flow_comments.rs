// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Margin comment panel layout for the paginated engine.
//!
//! Each `Inline::Comment` start anchor records a (comment id, content-local y)
//! pair on the current page. When the page is finished, every anchored comment
//! is laid out as a card in a gutter to the right of the page: a tinted
//! background, the author line, then the comment body. Cards are stacked
//! top-to-bottom and pushed down so they never overlap.

use loki_doc_model::NodeAttr;
use loki_doc_model::content::annotation::CommentRefKind;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::style::props::char_props::CharProps;
use loki_primitives::units::Points;

use super::FlowState;
use crate::color::LayoutColor;
use crate::geometry::LayoutRect;
use crate::items::{PositionedItem, PositionedRect};

/// Gap between the page's right edge and the comment gutter.
const GUTTER_GAP: f32 = 12.0;
/// Width of the comment gutter (card area). Derived so the gap + width match
/// the public [`crate::COMMENT_GUTTER_WIDTH`] the host reserves.
const GUTTER_WIDTH: f32 = crate::COMMENT_GUTTER_WIDTH - GUTTER_GAP;
/// Inner padding inside each comment card.
const CARD_PADDING: f32 = 6.0;
/// Vertical gap between stacked cards.
const CARD_GAP: f32 = 8.0;

/// Records the start anchors of any comments in `inlines` at the current cursor
/// position. Paginated mode only — the gutter panel is a print-layout feature.
pub(super) fn record_comment_anchors(state: &mut FlowState, inlines: &[Inline]) {
    if !state.mode.is_paginated() || state.comments.is_empty() {
        return;
    }
    for inline in inlines {
        if let Inline::Comment(c) = inline
            && c.kind == CommentRefKind::Start
        {
            state
                .pending_comment_anchors
                .push((c.id.clone(), state.cursor_y));
        }
    }
}

/// Lays out the comment cards for the anchors recorded on the page being
/// finished and returns the gutter items (page-local coordinates). Clears the
/// pending anchors.
pub(super) fn layout_comment_panel(state: &mut FlowState) -> Vec<PositionedItem> {
    let anchors = std::mem::take(&mut state.pending_comment_anchors);
    if anchors.is_empty() {
        return Vec::new();
    }
    let card_x = state.page_size.width + GUTTER_GAP;
    let inner_width = (GUTTER_WIDTH - 2.0 * CARD_PADDING).max(1.0);
    // Comment-card background tint (light yellow).
    let card_fill = LayoutColor::new(1.0, 0.97, 0.80, 1.0);

    let mut items = Vec::new();
    // Next free y in content-local coordinates (cards stack downward).
    let mut next_free_y = 0.0f32;
    for (id, anchor_y) in anchors {
        let Some(comment) = state.comments.iter().find(|c| c.id == id) else {
            continue;
        };

        // Author line (bold) followed by the comment body.
        let mut blocks: Vec<Block> = vec![Block::StyledPara(author_paragraph(
            comment.author.as_deref().unwrap_or("Comment"),
        ))];
        blocks.extend(comment.body.iter().cloned());

        let (mut card_items, height) = super::layout_blocks_reflow(
            state.resources,
            &blocks,
            state.catalog,
            inner_width,
            state.display_scale,
            None,
        );

        // Stack: never above the anchor, never overlapping the previous card.
        let top = anchor_y.max(next_free_y);
        let page_top = top + state.margins.top;

        items.push(PositionedItem::FilledRect(PositionedRect {
            rect: LayoutRect::new(card_x, page_top, GUTTER_WIDTH, height + 2.0 * CARD_PADDING),
            color: card_fill,
        }));
        for item in &mut card_items {
            item.translate(card_x + CARD_PADDING, page_top + CARD_PADDING);
        }
        items.append(&mut card_items);

        next_free_y = top + height + 2.0 * CARD_PADDING + CARD_GAP;
    }
    items
}

/// Builds a small bold paragraph carrying the comment author's name.
fn author_paragraph(author: &str) -> StyledParagraph {
    StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: Some(Box::new(CharProps {
            bold: Some(true),
            font_size: Some(Points::new(9.0)),
            ..Default::default()
        })),
        inlines: vec![Inline::Str(author.to_string())],
        attr: NodeAttr::default(),
    }
}
