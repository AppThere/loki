// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Flow engine — places blocks sequentially and handles page breaking.
//!
//! [`flow_section`] converts a stream of [`Block`]s into a flat list of
//! [`PositionedItem`]s. In paginated mode the engine splits content across
//! pages; the returned flat list offsets each page's items vertically by
//! `page_index × page_height` so renderers can treat it as a stacked canvas.

use loki_doc_model::content::block::StyledParagraph;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::{Block, NodeAttr, Section, StyleCatalog};

use crate::color::LayoutColor;
use crate::font::FontResources;
use crate::geometry::{LayoutInsets, LayoutRect, LayoutSize};
use crate::items::{PositionedItem, PositionedRect};
use crate::mode::LayoutMode;
use crate::para::layout_paragraph;
use crate::resolve::{flatten_paragraph, pts_to_f32, resolve_para_props};
use crate::result::LayoutPage;

// ── Public types ──────────────────────────────────────────────────────────────

/// Non-fatal layout issues collected during [`flow_section`].
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum LayoutWarning {
    /// A block was too tall to fit on any page in paginated mode.
    BlockExceedsPageHeight {
        /// 0-indexed position of the block in the section.
        block_index: usize,
        /// Measured block height in points.
        block_height: f32,
    },
    /// An image src could not be resolved by the renderer.
    UnresolvedImage {
        /// Source URL or data URI that could not be resolved.
        src: String,
    },
}

// ── Private flow state ────────────────────────────────────────────────────────

struct FlowState<'a> {
    resources: &'a mut FontResources,
    catalog: &'a StyleCatalog,
    mode: &'a LayoutMode,
    display_scale: f32,
    /// Current y within the current page content area (or canvas).
    cursor_y: f32,
    /// Available width for content (page width minus margins, or reflow width).
    content_width: f32,
    /// Items accumulating in the current page (or entire canvas for continuous).
    current_items: Vec<PositionedItem>,
    /// Completed pages (paginated mode only).
    pages: Vec<LayoutPage>,
    /// Physical page dimensions in points.
    page_size: LayoutSize,
    /// Content-area margins derived from the section's `PageLayout`.
    margins: LayoutInsets,
    /// Height of the content area within a page (page_height − v_margins).
    page_content_height: f32,
    /// 1-indexed current page number.
    page_number: usize,
    /// Whether the previous block set keep_with_next (reserved for Session 6).
    #[allow(dead_code)]
    pending_keep_with_next: bool,
    /// Accumulated warnings.
    warnings: Vec<LayoutWarning>,
    /// Accumulated horizontal indentation in points.
    current_indent: f32,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Flow all blocks from a section into positioned items.
///
/// Returns `(items, total_height, warnings)`.
///
/// For continuous modes (`Pageless`, `Reflow`) the items are absolute on a
/// single canvas and `total_height` is the total canvas height.
///
/// For `Paginated` mode items from page *n* (0-indexed) are translated by
/// `n × page_height + margins.top` vertically and `margins.left` horizontally
/// so the flat list represents a vertically stacked view of all pages.
/// `total_height` equals `page_count × page_height`.
pub fn flow_section(
    resources: &mut FontResources,
    section: &Section,
    catalog: &StyleCatalog,
    mode: &LayoutMode,
    display_scale: f32,
) -> (Vec<PositionedItem>, f32, Vec<LayoutWarning>) {
    let pl = &section.layout;
    let page_w = pts_to_f32(pl.page_size.width);
    let page_h = pts_to_f32(pl.page_size.height);
    let margins = LayoutInsets {
        top: pts_to_f32(pl.margins.top),
        right: pts_to_f32(pl.margins.right),
        bottom: pts_to_f32(pl.margins.bottom),
        left: pts_to_f32(pl.margins.left),
    };
    let content_width = match mode {
        LayoutMode::Reflow { available_width } => *available_width,
        _ => (page_w - margins.horizontal()).max(0.0),
    };

    let mut state = FlowState {
        resources,
        catalog,
        mode,
        display_scale,
        cursor_y: 0.0,
        content_width,
        current_items: Vec::new(),
        pages: Vec::new(),
        page_size: LayoutSize::new(page_w, page_h),
        margins,
        page_content_height: (page_h - margins.vertical()).max(0.0),
        page_number: 1,
        pending_keep_with_next: false,
        warnings: Vec::new(),
        current_indent: 0.0,
    };

    for (idx, block) in section.blocks.iter().enumerate() {
        flow_block(&mut state, block, idx);
    }

    if mode.is_paginated() {
        finish_page(&mut state);
        let total_height = state.pages.len() as f32 * page_h;
        let mut flat: Vec<PositionedItem> = Vec::new();
        for (i, page) in state.pages.iter().enumerate() {
            let dy = i as f32 * page_h + state.margins.top;
            let dx = state.margins.left;
            for item in &page.content_items {
                let mut item = item.clone();
                item.translate(dx, dy);
                flat.push(item);
            }
        }
        (flat, total_height, state.warnings)
    } else {
        if matches!(mode, LayoutMode::Pageless) {
            let dx = margins.left;
            for item in &mut state.current_items {
                item.translate(dx, 0.0);
            }
        }
        (state.current_items, state.cursor_y, state.warnings)
    }
}

// ── Block dispatch ────────────────────────────────────────────────────────────

fn flow_block(state: &mut FlowState, block: &Block, idx: usize) {
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
            state.current_indent += 18.0; // 1/4 inch indent
            for item in items {
                for (b_idx, b) in item.iter().enumerate() {
                    if b_idx == 0 {
                        // Prepend bullet to the first block if it's a paragraph
                        if let Block::StyledPara(p) = b {
                            let mut p = p.clone();
                            p.inlines.insert(0, Inline::Str("• ".into()));
                            flow_paragraph(state, &p, idx);
                            continue;
                        }
                    }
                    flow_block(state, b, idx);
                }
            }
            state.current_indent = old_indent;
        }
        Block::OrderedList(attrs, items) => {
            let old_indent = state.current_indent;
            state.current_indent += 18.0;
            for (i, item) in items.iter().enumerate() {
                let marker = format!("{}. ", attrs.start_number + i as i32);
                for (b_idx, b) in item.iter().enumerate() {
                    if b_idx == 0 {
                        if let Block::StyledPara(p) = b {
                            let mut p = p.clone();
                            p.inlines.insert(0, Inline::Str(marker.clone().into()));
                            flow_paragraph(state, &p, idx);
                            continue;
                        }
                    }
                    flow_block(state, b, idx);
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
        Block::Div(_, blocks) => {
            for b in blocks {
                flow_block(state, b, idx);
            }
        }
        Block::Figure(_, _, blocks) => {
            for b in blocks {
                flow_block(state, b, idx);
            }
        }
        Block::Table(tbl) => flow_table(state, tbl, idx),
        Block::HorizontalRule => flow_hrule(state),
        _ => {}               // CodeBlock, RawBlock, etc. — skip silently
    }
}

// ── Paragraph placement ───────────────────────────────────────────────────────

fn flow_paragraph(state: &mut FlowState, para: &StyledParagraph, block_index: usize) {
    let resolved = resolve_para_props(para, state.catalog);
    let (text, spans) = flatten_paragraph(para, state.catalog);

    // Apply space_before.
    state.cursor_y += resolved.space_before;

    // Honour explicit page break.
    if resolved.page_break_before && state.mode.is_paginated() {
        finish_page(state);
        // space_before is intentionally not re-applied after an explicit break.
    }

    let para_layout = layout_paragraph(
        state.resources,
        &text,
        &spans,
        &resolved,
        state.content_width,
        state.display_scale,
    );

    // Check if the paragraph fits on the current page (paginated mode only).
    if state.mode.is_paginated() {
        if para_layout.height > state.page_content_height {
            // Block is taller than a full page: warn and place anyway.
            state.warnings.push(LayoutWarning::BlockExceedsPageHeight {
                block_index,
                block_height: para_layout.height,
            });
        } else {
            let needed = para_layout.height + resolved.space_after;
            let available = state.page_content_height - state.cursor_y;
            if needed > available && state.cursor_y > 0.0 {
                finish_page(state);
                // Re-apply space_before on the new page.
                state.cursor_y += resolved.space_before;
            }
        }
    }

    // Translate items from paragraph-relative to page-content-area-relative.
    let dy = state.cursor_y;
    let dx = state.current_indent;
    for mut item in para_layout.items {
        item.translate(dx, dy);
        state.current_items.push(item);
    }
    state.cursor_y += para_layout.height + resolved.space_after;
}

// ── Page management ───────────────────────────────────────────────────────────

fn finish_page(state: &mut FlowState) {
    let page = LayoutPage {
        page_number: state.page_number,
        page_size: state.page_size,
        margins: state.margins,
        content_items: std::mem::take(&mut state.current_items),
        header_items: vec![], // TODO: Session 6 — headers/footers
        footer_items: vec![],
    };
    state.pages.push(page);
    state.page_number += 1;
    state.cursor_y = 0.0;
}

// ── Coordinate translation ────────────────────────────────────────────────────


// ── Miscellaneous block renderers ─────────────────────────────────────────────

fn flow_hrule(state: &mut FlowState) {
    const RULE_HEIGHT: f32 = 1.0;
    const RULE_SPACING: f32 = 6.0;
    state.current_items.push(PositionedItem::HorizontalRule(PositionedRect {
        rect: LayoutRect::new(0.0, state.cursor_y, state.content_width, RULE_HEIGHT),
        color: LayoutColor::BLACK,
    }));
    state.cursor_y += RULE_HEIGHT + RULE_SPACING;
}

// ── Paragraph synthesisers ────────────────────────────────────────────────────

fn synthesize_plain_para(inlines: &[Inline]) -> StyledParagraph {
    StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: inlines.to_vec(),
        attr: NodeAttr::default(),
    }
}

fn synthesize_heading_para(level: u8, attr: &NodeAttr, inlines: &[Inline]) -> StyledParagraph {
    use loki_doc_model::style::catalog::StyleId;
    use loki_doc_model::style::props::para_props::{ParagraphAlignment, ParaProps};
    let style_name = match level {
        1 => "Heading1",
        2 => "Heading2",
        3 => "Heading3",
        4 => "Heading4",
        5 => "Heading5",
        _ => "Heading6",
    };
    // Restore any alignment that was carried forward in NodeAttr.kv.
    let direct_alignment = attr.kv.iter().find(|(k, _)| k == "jc").and_then(|(_, v)| match v.as_str() {
        "center" => Some(ParagraphAlignment::Center),
        "right" => Some(ParagraphAlignment::Right),
        "justify" => Some(ParagraphAlignment::Justify),
        _ => None,
    });
    let direct_para_props = direct_alignment.map(|align| {
        Box::new(ParaProps { alignment: Some(align), ..Default::default() })
    });
    StyledParagraph {
        style_id: Some(StyleId::new(style_name)),
        direct_para_props,
        direct_char_props: None,
        inlines: inlines.to_vec(),
        attr: NodeAttr::default(),
    }
}

// ── Table layout ─────────────────────────────────────────────────────────────

fn flow_table(state: &mut FlowState, tbl: &loki_doc_model::content::table::core::Table, idx: usize) {
    let col_count = tbl.col_count().max(1);
    let col_w = state.content_width / col_count as f32;

    let mut rows = Vec::new();
    rows.extend(&tbl.head.rows);
    for body in &tbl.bodies {
        rows.extend(&body.head_rows);
        rows.extend(&body.body_rows);
    }
    rows.extend(&tbl.foot.rows);

    for row in rows {
        let row_y_start = state.cursor_y;
        let mut row_max_h = 0.0f32;

        for (c_idx, cell) in row.cells.iter().enumerate() {
            let old_indent = state.current_indent;
            let old_width = state.content_width;

            state.current_indent = old_indent + (c_idx as f32 * col_w);
            state.content_width = col_w;
            state.cursor_y = row_y_start;

            for block in &cell.blocks {
                flow_block(state, block, idx);
            }

            let cell_h = state.cursor_y - row_y_start;
            row_max_h = row_max_h.max(cell_h);

            state.current_indent = old_indent;
            state.content_width = old_width;
        }
        state.cursor_y = row_y_start + row_max_h;
    }
}

#[cfg(test)]
#[path = "flow_tests.rs"]
mod tests;
