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

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::layout::header_footer::HeaderFooter;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::style::list_style::ListId;
use loki_doc_model::{NodeAttr, Section, StyleCatalog};
use loki_primitives::units::Points;

use crate::LayoutOptions;
use crate::color::LayoutColor;
use crate::font::FontResources;
use crate::geometry::{LayoutInsets, LayoutPoint, LayoutRect, LayoutSize};
use crate::items::{PositionedBorderRect, PositionedItem, PositionedRect};
use crate::mode::LayoutMode;
use crate::resolve::{
    CollectedNote, convert_border, pts_to_f32, resolve_color, resolve_para_props,
};
use crate::result::{LayoutPage, PageEditingData, PageParagraphData};

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
    /// Layout options forwarded from the [`layout_document`] caller.
    pub(super) options: &'a LayoutOptions,
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
    /// Monotonically-increasing counter for footnotes and endnotes within
    /// the current section. Incremented by `walk_inlines` when a `Note` is met.
    pub(super) note_counter: u32,
    /// Footnotes and endnotes collected while flowing the current section.
    /// Rendered at the end of the section by `flow_footnotes`.
    pub(super) pending_footnotes: Vec<CollectedNote>,
    /// Paragraph metadata for the current page (block index, layout, origin).
    pub(super) current_paragraphs: Vec<PageParagraphData>,
}

impl<'a> FlowState<'a> {
    /// Advance the counter for `list_id` at `level` and return the new value.
    ///
    /// - Initialises the counter from `start_value` on first use.
    /// - Resets all deeper-level counters to 0 so they re-initialise from
    ///   their own `start_value` when next encountered.
    pub(super) fn advance_counter(&mut self, list_id: &ListId, level: u8, start_value: u32) -> u32 {
        let counters = self
            .list_counters
            .entry(list_id.clone())
            .or_insert([0u32; 9]);
        let lvl = level as usize;
        if counters[lvl] == 0 {
            counters[lvl] = start_value;
        } else {
            counters[lvl] += 1;
        }
        for counter in counters.iter_mut().take(9).skip(lvl + 1) {
            *counter = 0;
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
    options: &LayoutOptions,
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
        options,
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
        note_counter: 0,
        pending_footnotes: Vec::new(),
        current_paragraphs: Vec::new(),
    };

    if mode.is_paginated() {
        // In paginated mode, intercept keep_with_next chains at the top level
        // before dispatching to flow_block.
        let mut i = 0;
        while i < section.blocks.len() {
            let block = &section.blocks[i];
            if let Block::StyledPara(para) = block
                && resolve_para_props(para, catalog).keep_with_next
            {
                let consumed = flow_keep_with_next_chain(&mut state, &section.blocks, i);
                i += consumed;
                continue;
            }
            flow_block(&mut state, block, i);
            i += 1;
        }
    } else {
        for (idx, block) in section.blocks.iter().enumerate() {
            flow_block(&mut state, block, idx);
        }
    }

    flow_footnotes(&mut state);

    if mode.is_paginated() {
        finish_page(&mut state);
        FlowOutput::Pages {
            pages: state.pages,
            warnings: state.warnings,
        }
    } else {
        if matches!(mode, LayoutMode::Pageless) {
            let dx = margins.left;
            for item in &mut state.current_items {
                item.translate(dx, 0.0);
            }
        }
        FlowOutput::Canvas {
            items: state.current_items,
            height: state.cursor_y,
            warnings: state.warnings,
        }
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
}

// ── Header / footer layout helpers ───────────────────────────────────────────

/// Lay out `blocks` in reflow mode using `available_width`.
///
/// Returns the positioned items (in `(0,0)`-origin canvas coordinates) and the
/// total canvas height. Items have no Y offset applied — the caller translates
/// them to page-local coordinates.
fn layout_blocks_reflow(
    resources: &mut FontResources,
    blocks: &[Block],
    catalog: &StyleCatalog,
    available_width: f32,
    display_scale: f32,
) -> (Vec<PositionedItem>, f32) {
    use crate::LayoutOptions;
    let synthetic = Section {
        layout: PageLayout::default(),
        blocks: blocks.to_vec(),
        extensions: ExtensionBag::default(),
    };
    let mode = LayoutMode::Reflow { available_width };
    // Headers/footers are read-only; always use default (no editing overhead).
    let options = LayoutOptions::default();
    match flow_section(
        resources,
        &synthetic,
        catalog,
        &mode,
        display_scale,
        &options,
    ) {
        FlowOutput::Canvas { items, height, .. } => (items, height),
        FlowOutput::Pages { .. } => unreachable!("Reflow mode always returns Canvas"),
    }
}

/// Populate header/footer items for each page in `pages`.
///
/// Pre-lays-out all unique header/footer variants once (in reflow mode), then
/// assigns translated copies to each page. Items are translated to page-local
/// coordinates:
/// - Header top: `margins.header`
/// - Footer top: `page_height - margins.footer - footer_height`
pub(crate) fn assign_headers_footers(
    pages: &mut [LayoutPage],
    layout: &PageLayout,
    resources: &mut FontResources,
    catalog: &StyleCatalog,
    display_scale: f32,
) {
    // Pre-layout each variant that is present.
    let content_width = pages
        .first()
        .map(|p| (p.page_size.width - p.margins.horizontal()).max(0.0))
        .unwrap_or(0.0);

    let mut lay = |hf: &HeaderFooter| -> (Vec<PositionedItem>, f32) {
        layout_blocks_reflow(resources, &hf.blocks, catalog, content_width, display_scale)
    };

    let hdr_default: Option<(Vec<PositionedItem>, f32)> = layout.header.as_ref().map(&mut lay);
    let hdr_first: Option<(Vec<PositionedItem>, f32)> = layout.header_first.as_ref().map(&mut lay);
    let hdr_even: Option<(Vec<PositionedItem>, f32)> = layout.header_even.as_ref().map(&mut lay);
    let ftr_default: Option<(Vec<PositionedItem>, f32)> = layout.footer.as_ref().map(&mut lay);
    let ftr_first: Option<(Vec<PositionedItem>, f32)> = layout.footer_first.as_ref().map(&mut lay);
    let ftr_even: Option<(Vec<PositionedItem>, f32)> = layout.footer_even.as_ref().map(&mut lay);

    use crate::resolve::pts_to_f32;
    let hdr_margin_y = pts_to_f32(layout.margins.header);
    let ftr_margin = pts_to_f32(layout.margins.footer);
    let left_margin = pts_to_f32(layout.margins.left);

    for page in pages.iter_mut() {
        let page_h = page.page_size.height;
        let pn = page.page_number;

        let hdr = if pn == 1 && hdr_first.is_some() {
            hdr_first.as_ref()
        } else if pn % 2 == 0 && hdr_even.is_some() {
            hdr_even.as_ref()
        } else {
            hdr_default.as_ref()
        };

        let ftr = if pn == 1 && ftr_first.is_some() {
            ftr_first.as_ref()
        } else if pn % 2 == 0 && ftr_even.is_some() {
            ftr_even.as_ref()
        } else {
            ftr_default.as_ref()
        };

        if let Some((items, h)) = hdr {
            let mut translated: Vec<PositionedItem> = items.clone();
            for item in &mut translated {
                item.translate(left_margin, hdr_margin_y);
            }
            page.header_items = translated;
            page.header_height = *h;
        }

        if let Some((items, h)) = ftr {
            let footer_y = page_h - ftr_margin - h;
            let mut translated: Vec<PositionedItem> = items.clone();
            for item in &mut translated {
                item.translate(left_margin, footer_y);
            }
            page.footer_items = translated;
            page.footer_height = *h;
        }
    }
}

// ── Miscellaneous block renderers ─────────────────────────────────────────────

fn flow_hrule(state: &mut FlowState) {
    const RULE_HEIGHT: f32 = 1.0;
    const RULE_SPACING: f32 = 6.0;
    state
        .current_items
        .push(PositionedItem::HorizontalRule(PositionedRect {
            rect: LayoutRect::new(0.0, state.cursor_y, state.content_width, RULE_HEIGHT),
            color: LayoutColor::BLACK,
        }));
    state.cursor_y += RULE_HEIGHT + RULE_SPACING;
}

// ── Footnote rendering ────────────────────────────────────────────────────────

/// Render all accumulated footnotes at the end of the section.
///
/// Places a 1/3-width separator rule followed by each note body. The note
/// reference mark (e.g. "¹") is prepended to the first block of each note.
/// End-of-section placement is used for v0.1; end-of-page is deferred.
fn flow_footnotes(state: &mut FlowState) {
    if state.pending_footnotes.is_empty() {
        return;
    }
    let notes = std::mem::take(&mut state.pending_footnotes);

    // Separator: 1/3-width, 0.5 pt tall, 4 pt spacing above and below.
    const SEP_HEIGHT: f32 = 0.5;
    const SEP_GAP: f32 = 4.0;
    let sep_w = state.content_width / 3.0;
    state.cursor_y += SEP_GAP;
    state
        .current_items
        .push(PositionedItem::HorizontalRule(PositionedRect {
            rect: LayoutRect::new(0.0, state.cursor_y, sep_w, SEP_HEIGHT),
            color: LayoutColor::BLACK,
        }));
    state.cursor_y += SEP_HEIGHT + SEP_GAP;

    for note in notes {
        let mark = format!("{} ", &footnote_mark(note.number));
        let mut first = true;
        for block in &note.blocks {
            if first {
                first = false;
                if let Block::StyledPara(p) = block {
                    let mut p = p.clone();
                    p.inlines.insert(0, Inline::Str(mark.clone()));
                    flow_paragraph(state, &p, 0);
                    continue;
                }
            }
            flow_block(state, block, 0);
        }
    }
}

/// Return the Unicode superscript mark for note number `n`.
fn footnote_mark(n: u32) -> String {
    match n {
        1 => "\u{00B9}".to_string(),
        2 => "\u{00B2}".to_string(),
        3 => "\u{00B3}".to_string(),
        4 => "\u{2074}".to_string(),
        5 => "\u{2075}".to_string(),
        6 => "\u{2076}".to_string(),
        7 => "\u{2077}".to_string(),
        8 => "\u{2078}".to_string(),
        9 => "\u{2079}".to_string(),
        _ => format!("[{n}]"),
    }
}

// ── Paragraph synthesisers ────────────────────────────────────────────────────

pub(super) fn synthesize_plain_para(inlines: &[Inline]) -> StyledParagraph {
    StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: inlines.to_vec(),
        attr: NodeAttr::default(),
    }
}

pub(super) fn synthesize_heading_para(
    level: u8,
    attr: &NodeAttr,
    inlines: &[Inline],
) -> StyledParagraph {
    use loki_doc_model::style::catalog::StyleId;
    use loki_doc_model::style::props::para_props::{ParaProps, ParagraphAlignment};
    // Prefer the style name carried in NodeAttr (set by the ODF mapper from
    // text:style-name so the catalog can resolve ODF heading properties like
    // font-size and bold). Fall back to the canonical OOXML/internal names.
    let style_id: StyleId = attr
        .kv
        .iter()
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
    let direct_alignment =
        attr.kv
            .iter()
            .find(|(k, _)| k == "jc")
            .and_then(|(_, v)| match v.as_str() {
                "center" => Some(ParagraphAlignment::Center),
                "right" => Some(ParagraphAlignment::Right),
                "justify" => Some(ParagraphAlignment::Justify),
                _ => None,
            });
    let direct_para_props = direct_alignment.map(|align| {
        Box::new(ParaProps {
            alignment: Some(align),
            ..Default::default()
        })
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

fn get_items_max_x(items: &[PositionedItem]) -> f32 {
    let mut max_x = 0.0f32;
    for item in items {
        let x = match item {
            PositionedItem::GlyphRun(r) => {
                let mut run_max = r.origin.x;
                for g in &r.glyphs {
                    let right = r.origin.x + g.x + g.advance;
                    if right > run_max {
                        run_max = right;
                    }
                }
                run_max
            }
            PositionedItem::FilledRect(r) | PositionedItem::HorizontalRule(r) => {
                r.rect.origin.x + r.rect.size.width
            }
            PositionedItem::BorderRect(r) => r.rect.origin.x + r.rect.size.width,
            PositionedItem::Image(r) => r.rect.origin.x + r.rect.size.width,
            PositionedItem::Decoration(d) => d.x + d.width,
            PositionedItem::ClippedGroup { clip_rect, items } => {
                let inner_max = get_items_max_x(items);
                inner_max.min(clip_rect.origin.x + clip_rect.size.width)
            }
            PositionedItem::RotatedGroup {
                origin,
                content_width,
                ..
            } => origin.x + content_width,
        };
        if x > max_x {
            max_x = x;
        }
    }
    max_x
}

fn measure_cell_height(
    resources: &mut FontResources,
    catalog: &StyleCatalog,
    display_scale: f32,
    options: &LayoutOptions,
    cell: &loki_doc_model::content::table::row::Cell,
    cell_content_width: f32,
    idx: usize,
) -> f32 {
    use loki_doc_model::content::table::row::CellTextDirection;

    let pad_top = cell.props.padding_top.map(pts_to_f32).unwrap_or(0.0);
    let pad_bottom = cell.props.padding_bottom.map(pts_to_f32).unwrap_or(0.0);

    let is_rotated = matches!(
        cell.props.text_direction.as_ref(),
        Some(CellTextDirection::TbRl | CellTextDirection::TbLr | CellTextDirection::BtLr)
    );

    let flow_w = if is_rotated {
        10000.0
    } else {
        cell_content_width
    };

    let mut temp_state = FlowState {
        resources,
        catalog,
        mode: &LayoutMode::Pageless,
        display_scale,
        options,
        cursor_y: 0.0,
        content_width: flow_w,
        current_items: Vec::new(),
        pages: Vec::new(),
        page_size: LayoutSize::default(),
        margins: LayoutInsets::default(),
        page_content_height: 0.0,
        page_number: 1,
        warnings: Vec::new(),
        current_indent: 0.0,
        list_counters: HashMap::new(),
        prev_list_id: None,
        note_counter: 0,
        pending_footnotes: Vec::new(),
        current_paragraphs: Vec::new(),
    };

    for block in &cell.blocks {
        flow_block(&mut temp_state, block, idx);
    }

    if is_rotated {
        let max_x = get_items_max_x(&temp_state.current_items);
        max_x + pad_top + pad_bottom
    } else {
        let content_h = temp_state.cursor_y;
        content_h + pad_top + pad_bottom
    }
}

fn resolve_column_widths(
    state: &FlowState,
    tbl: &loki_doc_model::content::table::core::Table,
) -> Vec<f32> {
    use loki_doc_model::content::table::col::{ColWidth, TableWidth};

    let col_count = tbl.col_count().max(1);
    let table_width = match tbl.width.as_ref() {
        Some(TableWidth::Fixed(w)) => *w,
        Some(TableWidth::Percent(p)) => state.content_width * (p / 100.0),
        _ => state.content_width,
    };
    let table_width = table_width.max(0.0);

    let mut resolved_widths = vec![0.0f32; col_count];
    let mut proportional_shares = vec![0.0f32; col_count];
    let mut total_fixed_width = 0.0f32;
    let mut total_proportional_shares = 0.0f32;

    for i in 0..col_count {
        let spec = tbl.col_specs.get(i);
        let width_spec = spec.map(|s| s.width).unwrap_or(ColWidth::Default);
        match width_spec {
            ColWidth::Fixed(pts) => {
                let w = pts_to_f32(pts);
                resolved_widths[i] = w;
                total_fixed_width += w;
            }
            ColWidth::Proportional(share) => {
                proportional_shares[i] = share;
                total_proportional_shares += share;
            }
            ColWidth::Default | _ => {
                proportional_shares[i] = 1.0;
                total_proportional_shares += 1.0;
            }
        }
    }

    let remaining_width = (table_width - total_fixed_width).max(0.0);
    if total_proportional_shares > 0.0 {
        let share_unit = remaining_width / total_proportional_shares;
        for i in 0..col_count {
            let spec = tbl.col_specs.get(i);
            let width_spec = spec.map(|s| s.width).unwrap_or(ColWidth::Default);
            match width_spec {
                ColWidth::Proportional(_) | ColWidth::Default => {
                    resolved_widths[i] = proportional_shares[i] * share_unit;
                }
                _ => {}
            }
        }
    } else if total_fixed_width > 0.0 {
        let scale = table_width / total_fixed_width;
        for w in &mut resolved_widths {
            *w *= scale;
        }
    } else {
        let uniform_w = table_width / col_count as f32;
        resolved_widths.fill(uniform_w);
    }

    resolved_widths
}

// Helper to layout cell blocks inside a nested flow state.
// Helper requires passing all context values to configure the FlowState.
#[allow(clippy::too_many_arguments)]
fn flow_cell_blocks(
    resources: &mut FontResources,
    catalog: &StyleCatalog,
    display_scale: f32,
    options: &LayoutOptions,
    blocks: &[Block],
    content_width: f32,
    starting_indent: f32,
    starting_y: f32,
    idx: usize,
) -> Vec<PositionedItem> {
    let mut temp_state = FlowState {
        resources,
        catalog,
        mode: &LayoutMode::Pageless,
        display_scale,
        options,
        cursor_y: starting_y,
        content_width,
        current_items: Vec::new(),
        pages: Vec::new(),
        page_size: LayoutSize::default(),
        margins: LayoutInsets::default(),
        page_content_height: 0.0,
        page_number: 1,
        warnings: Vec::new(),
        current_indent: starting_indent,
        list_counters: HashMap::new(),
        prev_list_id: None,
        note_counter: 0,
        pending_footnotes: Vec::new(),
        current_paragraphs: Vec::new(),
    };

    for block in blocks {
        flow_block(&mut temp_state, block, idx);
    }

    temp_state.current_items
}

fn flow_table(
    state: &mut FlowState,
    tbl: &loki_doc_model::content::table::core::Table,
    idx: usize,
) {
    use loki_doc_model::content::table::row::{CellTextDirection, CellVerticalAlign};

    let col_widths = resolve_column_widths(state, tbl);

    let mut rows = Vec::new();
    rows.extend(&tbl.head.rows);
    for body in &tbl.bodies {
        rows.extend(&body.head_rows);
        rows.extend(&body.body_rows);
    }
    rows.extend(&tbl.foot.rows);

    let mut row_heights = vec![0.0f32; rows.len()];

    // Pass 1: Measure all cells with row_span == 1
    for (row_idx, row) in rows.iter().enumerate() {
        let mut col_start = 0;
        for cell in &row.cells {
            let col_end = (col_start + cell.col_span as usize).min(col_widths.len());
            if cell.row_span == 1 {
                let pad_left = cell.props.padding_left.map(pts_to_f32).unwrap_or(0.0);
                let pad_right = cell.props.padding_right.map(pts_to_f32).unwrap_or(0.0);
                let cell_w: f32 = col_widths[col_start..col_end].iter().sum();
                let cell_content_width = (cell_w - pad_left - pad_right).max(0.0);
                let h = measure_cell_height(
                    state.resources,
                    state.catalog,
                    state.display_scale,
                    state.options,
                    cell,
                    cell_content_width,
                    idx,
                );
                row_heights[row_idx] = row_heights[row_idx].max(h);
            }
            col_start = col_end;
        }
        row_heights[row_idx] = row_heights[row_idx].max(crate::MIN_ROW_HEIGHT);
    }

    // Pass 2: Distribute spanning cell heights across spanned rows
    for (row_idx, row) in rows.iter().enumerate() {
        let mut col_start = 0;
        for cell in &row.cells {
            let col_end = (col_start + cell.col_span as usize).min(col_widths.len());
            if cell.row_span > 1 {
                let span = cell.row_span as usize;
                let spanned_height: f32 = row_heights
                    [row_idx..(row_idx + span).min(row_heights.len())]
                    .iter()
                    .sum();
                let pad_left = cell.props.padding_left.map(pts_to_f32).unwrap_or(0.0);
                let pad_right = cell.props.padding_right.map(pts_to_f32).unwrap_or(0.0);
                let cell_w: f32 = col_widths[col_start..col_end].iter().sum();
                let cell_content_width = (cell_w - pad_left - pad_right).max(0.0);
                let needed = measure_cell_height(
                    state.resources,
                    state.catalog,
                    state.display_scale,
                    state.options,
                    cell,
                    cell_content_width,
                    idx,
                );
                if needed > spanned_height {
                    let extra = needed - spanned_height;
                    let last = (row_idx + span - 1).min(row_heights.len() - 1);
                    row_heights[last] += extra;
                }
            }
            col_start = col_end;
        }
    }

    // Pass 3: Place and flow cell blocks
    for (row_idx, row) in rows.iter().enumerate() {
        let row_max_h = row_heights[row_idx];

        if state.mode.is_paginated() {
            let remaining_h = state.page_content_height - state.cursor_y;
            if row_max_h > remaining_h && row_max_h <= state.page_content_height {
                finish_page(state);
            }
        }

        let original_row_page = state.page_number;
        let original_row_y_start = state.cursor_y;
        let mut row_y_start = original_row_y_start;
        let mut row_page = original_row_page;

        let table_indent = state.current_indent;
        let mut cell_starts = Vec::new();

        // Pass 3a: Flow cell content blocks
        let mut col_start = 0;
        for (c_idx, cell) in row.cells.iter().enumerate() {
            let col_end = (col_start + cell.col_span as usize).min(col_widths.len());
            let old_indent = state.current_indent;
            let old_width = state.content_width;

            let pad_top = cell.props.padding_top.map(pts_to_f32).unwrap_or(0.0);
            let pad_bottom = cell.props.padding_bottom.map(pts_to_f32).unwrap_or(0.0);
            let pad_left = cell.props.padding_left.map(pts_to_f32).unwrap_or(0.0);
            let pad_right = cell.props.padding_right.map(pts_to_f32).unwrap_or(0.0);

            let cell_w: f32 = col_widths[col_start..col_end].iter().sum();
            let cell_x = old_indent + col_widths[0..col_start].iter().sum::<f32>();
            let cell_content_width = (cell_w - pad_left - pad_right).max(0.0);

            let cell_height = if cell.row_span == 1 {
                row_max_h
            } else {
                let span = cell.row_span as usize;
                row_heights[row_idx..(row_idx + span).min(row_heights.len())]
                    .iter()
                    .sum()
            };

            // If a previous cell caused a page break, update row_y_start to the
            // top of the new page so this cell doesn't land in the wrong position.
            if state.page_number != row_page {
                row_y_start = state.cursor_y;
                row_page = state.page_number;
            }

            if state.page_number == original_row_page {
                state.cursor_y = original_row_y_start + pad_top;
            } else {
                state.cursor_y = 0.0 + pad_top;
            }

            cell_starts.push((state.page_number, state.current_items.len()));

            let rotation_degrees = match cell.props.text_direction.as_ref() {
                Some(CellTextDirection::TbRl) => Some(90.0_f32),
                Some(CellTextDirection::TbLr) => Some(270.0_f32),
                Some(CellTextDirection::BtLr) => Some(270.0_f32),
                _ => None,
            };

            let cell_items = if let Some(degrees) = rotation_degrees {
                // NOTE(cell-rotation): for rotated cells, content is laid out
                // with width/height swapped, then the RotatedGroup transform
                // visually rotates the result into the correct orientation.
                // This approximation works for text runs but may not be pixel-
                // perfect for complex mixed content.
                let rotated_content_width = (cell_height - pad_top - pad_bottom).max(0.0);
                let inner_items = flow_cell_blocks(
                    state.resources,
                    state.catalog,
                    state.display_scale,
                    state.options,
                    &cell.blocks,
                    rotated_content_width,
                    pad_top,
                    pad_left,
                    idx,
                );

                let max_x = get_items_max_x(&inner_items);
                let content_visual_height = max_x;
                let cell_avail_h = (cell_height - pad_top - pad_bottom).max(0.0);
                let extra_space = (cell_avail_h - content_visual_height).max(0.0);
                let y_offset = match cell.props.vertical_align {
                    Some(CellVerticalAlign::Middle) => extra_space / 2.0,
                    Some(CellVerticalAlign::Bottom) => extra_space,
                    _ => 0.0,
                };

                vec![PositionedItem::RotatedGroup {
                    origin: LayoutPoint {
                        x: cell_x,
                        y: row_y_start + y_offset,
                    },
                    degrees,
                    content_width: cell_height,
                    content_height: cell_content_width,
                    items: inner_items,
                }]
            } else {
                state.current_indent = cell_x + pad_left;
                state.content_width = cell_content_width;

                for block in &cell.blocks {
                    flow_block(state, block, idx);
                }

                // If it fits on a single page, apply vertical alignment
                let cell_page_start = cell_starts[c_idx].0;
                let cell_item_start = cell_starts[c_idx].1;
                if cell_page_start == state.page_number {
                    let content_h = (state.cursor_y - (row_y_start + pad_top)).max(0.0);
                    let cell_avail_h = (cell_height - pad_top - pad_bottom).max(0.0);
                    let extra_space = (cell_avail_h - content_h).max(0.0);
                    let y_offset = match cell.props.vertical_align {
                        Some(CellVerticalAlign::Middle) => extra_space / 2.0,
                        Some(CellVerticalAlign::Bottom) => extra_space,
                        _ => 0.0,
                    };
                    if y_offset > 0.0 {
                        for item in &mut state.current_items[cell_item_start..] {
                            item.translate(0.0, y_offset);
                        }
                    }
                }

                Vec::new()
            };

            for item in cell_items {
                state.current_items.push(item);
            }

            state.current_indent = old_indent;
            state.content_width = old_width;
            col_start = col_end;
        }

        let row_page_end = state.page_number;
        let row_y_end = if original_row_page == row_page_end {
            original_row_y_start + row_max_h
        } else {
            let first_h = (state.page_content_height - original_row_y_start).max(0.0);
            let intermediate_h =
                (row_page_end - original_row_page - 1) as f32 * state.page_content_height;
            (row_max_h - first_h - intermediate_h).max(0.0)
        };

        // Helper closures to calculate heights and Y coordinates of cell portions per page
        let get_cell_height_on_page = |p: usize, cell_page_start: usize, cell_h: f32| -> f32 {
            if p == cell_page_start {
                if p == row_page_end {
                    cell_h
                } else {
                    let y_start = if p == original_row_page {
                        original_row_y_start
                    } else {
                        0.0
                    };
                    (state.page_content_height - y_start).max(0.0)
                }
            } else if p == row_page_end {
                let start_y = if cell_page_start == original_row_page {
                    original_row_y_start
                } else {
                    0.0
                };
                let first_h = (state.page_content_height - start_y).max(0.0);
                let intermediate_h =
                    (row_page_end - cell_page_start - 1) as f32 * state.page_content_height;
                (cell_h - first_h - intermediate_h).max(0.0)
            } else {
                state.page_content_height
            }
        };

        let get_cell_y_on_page = |p: usize| -> f32 {
            if p == original_row_page {
                original_row_y_start
            } else {
                0.0
            }
        };

        // Pass 3b: Emit background and border decorations for this row's cells
        for p in original_row_page..=row_page_end {
            let mut col_start_map = Vec::new();
            {
                let mut curr_col = 0;
                for cell in &row.cells {
                    col_start_map.push(curr_col);
                    curr_col = (curr_col + cell.col_span as usize).min(col_widths.len());
                }
            }

            for (c_idx, cell) in row.cells.iter().enumerate().rev() {
                let cell_page_start = cell_starts[c_idx].0;
                let cell_item_start = cell_starts[c_idx].1;

                if p < cell_page_start {
                    continue;
                }

                let cell_h = if cell.row_span == 1 {
                    row_max_h
                } else {
                    let span = cell.row_span as usize;
                    row_heights[row_idx..(row_idx + span).min(row_heights.len())]
                        .iter()
                        .sum()
                };

                let h = get_cell_height_on_page(p, cell_page_start, cell_h);
                if h < 0.0 || (h == 0.0 && cell_h > 0.0) {
                    continue;
                }

                let y = get_cell_y_on_page(p);
                let col_start = col_start_map[c_idx];
                let col_end = (col_start + cell.col_span as usize).min(col_widths.len());
                let cell_w: f32 = col_widths[col_start..col_end].iter().sum();
                let cell_x = table_indent + col_widths[0..col_start].iter().sum::<f32>();
                let cell_rect = LayoutRect {
                    origin: LayoutPoint { x: cell_x, y },
                    size: LayoutSize {
                        width: cell_w,
                        height: h,
                    },
                };

                let has_borders = cell.props.border_top.is_some()
                    || cell.props.border_bottom.is_some()
                    || cell.props.border_left.is_some()
                    || cell.props.border_right.is_some();

                let is_first = p == cell_page_start;
                let is_last = p == row_page_end;

                let border_top = if is_first {
                    cell.props.border_top.as_ref().and_then(convert_border)
                } else {
                    None
                };
                let border_bottom = if is_last {
                    cell.props.border_bottom.as_ref().and_then(convert_border)
                } else {
                    None
                };
                let border_left = cell.props.border_left.as_ref().and_then(convert_border);
                let border_right = cell.props.border_right.as_ref().and_then(convert_border);

                let insert_idx = if p == cell_page_start {
                    cell_item_start
                } else {
                    0
                };

                if p == state.page_number {
                    if has_borders {
                        state.current_items.insert(
                            insert_idx,
                            PositionedItem::BorderRect(PositionedBorderRect {
                                rect: cell_rect,
                                top: border_top,
                                bottom: border_bottom,
                                left: border_left,
                                right: border_right,
                            }),
                        );
                    }
                    if let Some(bg) = cell.props.background_color.as_ref() {
                        state.current_items.insert(
                            insert_idx,
                            PositionedItem::FilledRect(PositionedRect {
                                rect: cell_rect,
                                color: resolve_color(Some(bg)),
                            }),
                        );
                    }
                } else if let Some(page) = state.pages.get_mut(p - 1) {
                    if has_borders {
                        page.content_items.insert(
                            insert_idx,
                            PositionedItem::BorderRect(PositionedBorderRect {
                                rect: cell_rect,
                                top: border_top,
                                bottom: border_bottom,
                                left: border_left,
                                right: border_right,
                            }),
                        );
                    }
                    if let Some(bg) = cell.props.background_color.as_ref() {
                        page.content_items.insert(
                            insert_idx,
                            PositionedItem::FilledRect(PositionedRect {
                                rect: cell_rect,
                                color: resolve_color(Some(bg)),
                            }),
                        );
                    }
                }
            }
        }

        state.cursor_y = row_y_end;
    }
}

#[cfg(test)]
#[path = "flow_tests.rs"]
mod tests;
