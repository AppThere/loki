// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Flow engine — places blocks sequentially and handles page breaking.
//!
//! [`flow_section`] converts a stream of [`Block`]s into positioned items.
//! In paginated mode the engine splits paragraphs at Parley line boundaries
//! and uses [`PositionedItem::ClippedGroup`] to render each page fragment
//! correctly. Page objects are built directly (no re-binning pass).
//!
//! Paragraph placement, splitting, and keep-with-next chain logic live in
//! the `para_impl` submodule (`flow_para.rs`).

#[path = "flow_para.rs"]
mod para_impl;

use std::collections::HashMap;

use loki_doc_model::content::block::StyledParagraph;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::style::list_style::ListId;
use loki_doc_model::{Block, NodeAttr, Section, StyleCatalog};

use crate::color::LayoutColor;
use crate::font::FontResources;
use crate::geometry::{LayoutInsets, LayoutRect, LayoutSize};
use crate::items::{PositionedItem, PositionedRect};
use crate::mode::LayoutMode;
use crate::resolve::{pts_to_f32, resolve_para_props};
use crate::result::LayoutPage;

use para_impl::{flow_keep_with_next_chain, flow_paragraph};

// ── Public types ──────────────────────────────────────────────────────────────

/// Output of [`flow_section`], discriminated by layout mode.
pub enum FlowOutput {
    /// Returned when `mode.is_paginated()`.
    ///
    /// Item origins in each page are relative to the page content-area
    /// top-left `(0, 0)` — no further translation is needed by the caller.
    Pages {
        /// Completed pages with content items in page-local coordinates.
        pages: Vec<LayoutPage>,
        /// Non-fatal warnings collected during layout.
        warnings: Vec<LayoutWarning>,
    },
    /// Returned for `Pageless` and `Reflow` modes.
    Canvas {
        /// All positioned items on the single canvas.
        items: Vec<PositionedItem>,
        /// Total canvas height in points.
        height: f32,
        /// Non-fatal warnings collected during layout.
        warnings: Vec<LayoutWarning>,
    },
}

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
    /// A `keep_together` paragraph was split because it exceeds full page
    /// height. The block could not be kept together on any single page.
    KeepTogetherOverride {
        /// 0-indexed position of the block in the section.
        block_index: usize,
        /// Measured block height in points.
        block_height: f32,
    },
    /// A `keep_with_next` chain was truncated at the chain limit of 5 blocks.
    KeepWithNextChainTruncated {
        /// 0-indexed position of the first block in the chain.
        start_block: usize,
        /// Number of blocks in the chain before truncation.
        chain_length: usize,
    },
    /// A `keep_with_next` chain was too tall to fit on one page; the chain
    /// was broken at the last block that fits.
    KeepWithNextChainTooTall {
        /// 0-indexed position of the first block in the chain.
        start_block: usize,
        /// Index of the block where the chain was broken.
        break_at: usize,
    },
}

// ── Private flow state ────────────────────────────────────────────────────────

pub(super) struct FlowState<'a> {
    pub(super) resources: &'a mut FontResources,
    pub(super) catalog: &'a StyleCatalog,
    pub(super) mode: &'a LayoutMode,
    pub(super) display_scale: f32,
    /// Current y within the current page content area (or canvas).
    pub(super) cursor_y: f32,
    /// Available width for content.
    pub(super) content_width: f32,
    /// Items accumulating in the current page (or entire canvas for continuous).
    pub(super) current_items: Vec<PositionedItem>,
    /// Completed pages (paginated mode only).
    pub(super) pages: Vec<LayoutPage>,
    /// Physical page dimensions in points.
    pub(super) page_size: LayoutSize,
    /// Content-area margins derived from the section's `PageLayout`.
    pub(super) margins: LayoutInsets,
    /// Height of the content area within a page (page_height − v_margins).
    pub(super) page_content_height: f32,
    /// 1-indexed current page number.
    pub(super) page_number: usize,
    /// Accumulated warnings.
    pub(super) warnings: Vec<LayoutWarning>,
    /// Accumulated horizontal indentation in points.
    pub(super) current_indent: f32,
    /// Per-list counter arrays. Each entry maps a `ListId` to a 9-element
    /// array where index = level (0-based) and value = current counter (1-based
    /// after first advance, 0 = not yet initialised).
    pub(super) list_counters: HashMap<ListId, [u32; 9]>,
    /// The `ListId` of the most recently placed list item, used to detect
    /// list-id changes and reset counters for the new list.
    pub(super) prev_list_id: Option<ListId>,
}

impl<'a> FlowState<'a> {
    /// Advance the counter for `list_id` at `level` and return the new value.
    ///
    /// - Initialises the counter from `start_value` on first use.
    /// - Resets all deeper-level counters to 0 so they re-initialise from
    ///   their own `start_value` when next encountered.
    pub(super) fn advance_counter(
        &mut self,
        list_id: &ListId,
        level: u8,
        start_value: u32,
    ) -> u32 {
        let counters = self.list_counters.entry(list_id.clone()).or_insert([0u32; 9]);
        let lvl = level as usize;
        if counters[lvl] == 0 {
            counters[lvl] = start_value;
        } else {
            counters[lvl] += 1;
        }
        for deeper in (lvl + 1)..9 {
            counters[deeper] = 0;
        }
        counters[lvl]
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Flow all blocks from a section into positioned items.
///
/// Returns a [`FlowOutput`] discriminated by layout mode:
///
/// - [`FlowOutput::Pages`]: each page's items are in page-content-area-local
///   coordinates (origin `(0, 0)` at the content-area top-left). The `margins`
///   field on each [`LayoutPage`] carries the insets. No further translation
///   by the caller is needed.
/// - [`FlowOutput::Canvas`]: all items on a single canvas. In `Pageless` mode
///   items are offset by `margins.left`; in `Reflow` mode there is no offset.
pub fn flow_section(
    resources: &mut FontResources,
    section: &Section,
    catalog: &StyleCatalog,
    mode: &LayoutMode,
    display_scale: f32,
) -> FlowOutput {
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
        warnings: Vec::new(),
        current_indent: 0.0,
        list_counters: HashMap::new(),
        prev_list_id: None,
    };

    if mode.is_paginated() {
        // In paginated mode, intercept keep_with_next chains at the top level
        // before dispatching to flow_block.
        let mut i = 0;
        while i < section.blocks.len() {
            let block = &section.blocks[i];
            if let Block::StyledPara(para) = block {
                if resolve_para_props(para, catalog).keep_with_next {
                    let consumed =
                        flow_keep_with_next_chain(&mut state, &section.blocks, i);
                    i += consumed;
                    continue;
                }
            }
            flow_block(&mut state, block, i);
            i += 1;
        }
    } else {
        for (idx, block) in section.blocks.iter().enumerate() {
            flow_block(&mut state, block, idx);
        }
    }

    if mode.is_paginated() {
        finish_page(&mut state);
        FlowOutput::Pages { pages: state.pages, warnings: state.warnings }
    } else {
        if matches!(mode, LayoutMode::Pageless) {
            let dx = margins.left;
            for item in &mut state.current_items {
                item.translate(dx, 0.0);
            }
        }
        FlowOutput::Canvas { items: state.current_items, height: state.cursor_y, warnings: state.warnings }
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
            state.current_indent += 18.0;
            for item in items {
                for (b_idx, b) in item.iter().enumerate() {
                    if b_idx == 0 {
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
        _ => {}
    }
}

// ── Page management ───────────────────────────────────────────────────────────

pub(super) fn finish_page(state: &mut FlowState) {
    let page = LayoutPage {
        page_number: state.page_number,
        page_size: state.page_size,
        margins: state.margins,
        content_items: std::mem::take(&mut state.current_items),
        header_items: vec![],
        footer_items: vec![],
    };
    state.pages.push(page);
    state.page_number += 1;
    state.cursor_y = 0.0;
}

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
    // Prefer the style name carried in NodeAttr (set by the ODF mapper from
    // text:style-name so the catalog can resolve ODF heading properties like
    // font-size and bold). Fall back to the canonical OOXML/internal names.
    let style_id: StyleId = attr.kv.iter()
        .find(|(k, _)| k == "style")
        .map(|(_, v)| StyleId::new(v.as_str()))
        .unwrap_or_else(|| {
            let hardcoded = match level {
                1 => "Heading1",
                2 => "Heading2",
                3 => "Heading3",
                4 => "Heading4",
                5 => "Heading5",
                _ => "Heading6",
            };
            StyleId::new(hardcoded)
        });
    let direct_alignment = attr.kv.iter().find(|(k, _)| k == "jc").and_then(|(_, v)| {
        match v.as_str() {
            "center" => Some(ParagraphAlignment::Center),
            "right" => Some(ParagraphAlignment::Right),
            "justify" => Some(ParagraphAlignment::Justify),
            _ => None,
        }
    });
    let direct_para_props = direct_alignment.map(|align| {
        Box::new(ParaProps { alignment: Some(align), ..Default::default() })
    });
    StyledParagraph {
        style_id: Some(style_id),
        direct_para_props,
        direct_char_props: None,
        inlines: inlines.to_vec(),
        attr: NodeAttr::default(),
    }
}

// ── Table layout ─────────────────────────────────────────────────────────────

fn flow_table(
    state: &mut FlowState,
    tbl: &loki_doc_model::content::table::core::Table,
    idx: usize,
) {
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
