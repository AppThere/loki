// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Miscellaneous flow helpers: horizontal rule, footnotes, para synthesisers,
//! and header/footer layout.

use loki_doc_model::StyleCatalog;
use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::block::StyledParagraph;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::layout::header_footer::HeaderFooter;
use loki_doc_model::layout::page::PageLayout;
use loki_doc_model::{NodeAttr, Section};

use crate::LayoutOptions;
use crate::color::LayoutColor;
use crate::flow::{FlowOutput, FlowState, flow_section};
use crate::flow_block::flow_block;
use crate::font::FontResources;
use crate::geometry::LayoutRect;
use crate::items::{PositionedItem, PositionedRect};
use crate::mode::LayoutMode;
use crate::resolve::pts_to_f32;
use crate::result::LayoutPage;

// ── Horizontal rule ───────────────────────────────────────────────────────────

pub(crate) fn flow_hrule(state: &mut FlowState) {
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
pub(crate) fn flow_footnotes(state: &mut FlowState) {
    if state.pending_footnotes.is_empty() {
        return;
    }
    let notes = std::mem::take(&mut state.pending_footnotes);

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

    use crate::flow::para_impl::flow_paragraph;
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
pub(crate) fn footnote_mark(n: u32) -> String {
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

pub(crate) fn synthesize_plain_para(inlines: &[Inline]) -> StyledParagraph {
    StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: inlines.to_vec(),
        attr: NodeAttr::default(),
    }
}

pub(crate) fn synthesize_heading_para(
    level: u8,
    attr: &NodeAttr,
    inlines: &[Inline],
) -> StyledParagraph {
    use loki_doc_model::style::catalog::StyleId;
    use loki_doc_model::style::props::para_props::{ParaProps, ParagraphAlignment};
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

// ── Header / footer layout ────────────────────────────────────────────────────

/// Lay out `blocks` in reflow mode using `available_width`.
fn layout_blocks_reflow(
    resources: &mut FontResources,
    blocks: &[Block],
    catalog: &StyleCatalog,
    available_width: f32,
    display_scale: f32,
) -> (Vec<PositionedItem>, f32) {
    let synthetic = Section {
        layout: PageLayout::default(),
        blocks: blocks.to_vec(),
        extensions: ExtensionBag::default(),
    };
    let mode = LayoutMode::Reflow { available_width };
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
pub(crate) fn assign_headers_footers(
    pages: &mut [LayoutPage],
    layout: &PageLayout,
    resources: &mut FontResources,
    catalog: &StyleCatalog,
    display_scale: f32,
) {
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
