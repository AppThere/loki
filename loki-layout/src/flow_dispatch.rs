// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Block dispatch and page finalization, split out of `flow.rs` for the
//! 300-line ceiling: `flow_block` routes each block variant to its handler
//! (lists synthesize marker paragraphs inline), `flow_blocks` recurses child
//! bodies, and `finish_page` positions columns and pushes a `LayoutPage`.
//! `flow_block` / `finish_page` are re-exported from `flow.rs`.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_primitives::units::Points;

use crate::result::{LayoutPage, PageEditingData};

use super::{
    FlowState, columns_impl, comments_impl, float_impl, flow_hrule, flow_paragraph, para_between,
    synthesize_heading_para, synthesize_plain_para, table_main,
};

// ── Block dispatch ────────────────────────────────────────────────────────────

pub(crate) fn flow_block(state: &mut FlowState, block: &Block, idx: usize) {
    // Only consecutive plain paragraphs continue a cross-paragraph float wrap;
    // any other block clears the float (reserving its remaining height) so it
    // does not overlap the image.
    if !matches!(
        block,
        Block::StyledPara(_) | Block::Para(_) | Block::Plain(_) | Block::Heading(..)
    ) {
        float_impl::reserve_active_float(state);
    }
    match block {
        Block::StyledPara(p) => flow_paragraph(state, p, idx),
        Block::Para(i) | Block::Plain(i) => {
            flow_paragraph(state, &synthesize_plain_para(i), idx);
        }
        Block::Heading(lvl, attr, i) => {
            flow_paragraph(state, &synthesize_heading_para(*lvl, attr, i), idx);
        }
        Block::BulletList(items) => {
            let old_indent = state.current_indent;
            let list_indent = old_indent + 18.0;
            for item in items {
                for (b_idx, b) in item.iter().enumerate() {
                    if b_idx == 0
                        && let Block::StyledPara(p) = b
                    {
                        let mut p = p.clone();
                        p.inlines.insert(0, Inline::Str("•\t".into()));
                        let mut direct = p.direct_para_props.take().unwrap_or_default();
                        direct.indent_hanging = Some(Points::new(18.0));
                        direct.indent_start = Some(Points::new(list_indent as f64));
                        p.direct_para_props = Some(direct);

                        let prev_indent = state.current_indent;
                        state.current_indent = 0.0;
                        flow_paragraph(state, &p, idx);
                        state.current_indent = prev_indent;
                        continue;
                    }
                    let prev_indent = state.current_indent;
                    state.current_indent = list_indent;
                    flow_block(state, b, idx);
                    state.current_indent = prev_indent;
                }
            }
            state.current_indent = old_indent;
        }
        Block::OrderedList(attrs, items) => {
            let old_indent = state.current_indent;
            let list_indent = old_indent + 18.0;
            for (i, item) in items.iter().enumerate() {
                let marker = format!("{}.\t", attrs.start_number + i as i32);
                for (b_idx, b) in item.iter().enumerate() {
                    if b_idx == 0
                        && let Block::StyledPara(p) = b
                    {
                        let mut p = p.clone();
                        p.inlines.insert(0, Inline::Str(marker.clone()));
                        let mut direct = p.direct_para_props.take().unwrap_or_default();
                        direct.indent_hanging = Some(Points::new(18.0));
                        direct.indent_start = Some(Points::new(list_indent as f64));
                        p.direct_para_props = Some(direct);

                        let prev_indent = state.current_indent;
                        state.current_indent = 0.0;
                        flow_paragraph(state, &p, idx);
                        state.current_indent = prev_indent;
                        continue;
                    }
                    let prev_indent = state.current_indent;
                    state.current_indent = list_indent;
                    flow_block(state, b, idx);
                    state.current_indent = prev_indent;
                }
            }
            state.current_indent = old_indent;
        }
        Block::BlockQuote(blocks) => {
            let old_indent = state.current_indent;
            state.current_indent += 18.0;
            for b in blocks {
                flow_block(state, b, idx);
            }
            state.current_indent = old_indent;
        }
        Block::Div(_, blocks) | Block::Figure(_, _, blocks) => flow_blocks(state, blocks, idx),
        Block::Table(tbl) => table_main::flow_table(state, tbl, idx),
        Block::HorizontalRule => flow_hrule(state),
        Block::TableOfContents(toc) => flow_blocks(state, &toc.body, idx),
        Block::Index(index) => flow_blocks(state, &index.body, idx),
        _ => {}
    }
}

/// Flows child blocks at the parent's `idx` (Div/Figure bodies, TOC/index snapshots).
fn flow_blocks(state: &mut FlowState, blocks: &[Block], idx: usize) {
    for (i, b) in blocks.iter().enumerate() {
        state.staged_between = para_between::stage(blocks, i, state.catalog);
        flow_block(state, b, idx);
    }
}

// ── Page management ───────────────────────────────────────────────────────────

pub(crate) fn finish_page(state: &mut FlowState) {
    // Position + separate the used columns, then reset for the next page.
    columns_impl::position_current_column(state);
    columns_impl::emit_column_separators(state);
    state.col_index = 0;
    state.column_top_y = 0.0;
    state.column_item_start = 0;
    state.column_para_start = 0;

    // Lay out the gutter comment panel for any comments anchored on this page.
    let comment_items = comments_impl::layout_comment_panel(state);

    let page = LayoutPage {
        page_number: state.page_number,
        page_size: state.page_size,
        margins: crate::paginate_blanks::mirrored_margins(
            state.margins,
            state.page_number,
            state.options.mirror_margins,
        ),
        content_items: std::mem::take(&mut state.current_items),
        header_items: vec![],
        footer_items: vec![],
        comment_items,
        header_height: 0.0,
        footer_height: 0.0,
        editing_data: if state.options.preserve_for_editing {
            Some(PageEditingData {
                paragraphs: state.current_paragraphs.clone(),
            })
        } else {
            None
        },
    };
    state.pages.push(page);
    state.page_number += 1;
    state.current_paragraphs.clear();
    state.cursor_y = 0.0;
    // Cross-paragraph float wrap does not continue onto the next page.
    state.active_float = None;
}
