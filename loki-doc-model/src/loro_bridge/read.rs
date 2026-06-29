// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Deserialization: Loro CRDT containers → Loki document model.

use super::BridgeError;
use super::inlines_read::reconstruct_inlines;
use super::props_read::{
    get_bool_from_map, get_f64_from_map, get_i64_from_map, get_str_from_map,
    reconstruct_char_props_from_map, reconstruct_para_props,
};
use crate::content::attr::NodeAttr;
use crate::content::block::Block;
use crate::layout::header_footer::{HeaderFooter, HeaderFooterKind};
use crate::layout::page::{PageLayout, PageMargins, PageOrientation, PageSize, SectionColumns};
use crate::loro_schema::*;
use loki_primitives::units::Points;
use loro::LoroMap;

// ── Block deserialization ─────────────────────────────────────────────────────

pub(super) fn map_loro_block(map: &LoroMap) -> Result<Block, BridgeError> {
    let block_type = map
        .get(KEY_TYPE)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())
        .unwrap_or_default();

    match block_type.as_str() {
        BLOCK_TYPE_PARA => Ok(Block::Para(reconstruct_inlines(map)?)),
        BLOCK_TYPE_STYLED_PARA => {
            let inlines = reconstruct_inlines(map)?;
            let style_id = map
                .get("style_id")
                .and_then(|v| v.into_value().ok())
                .and_then(|v| v.into_string().ok())
                .map(|s| crate::style::catalog::StyleId(s.to_string()));
            let direct_para_props = reconstruct_para_props(map).map(Box::new);
            let direct_char_props = reconstruct_char_props_from_map(map).map(Box::new);
            Ok(Block::StyledPara(crate::content::block::StyledParagraph {
                style_id,
                direct_para_props,
                direct_char_props,
                inlines,
                attr: NodeAttr::default(),
            }))
        }
        BLOCK_TYPE_HEADING => {
            let inlines = reconstruct_inlines(map)?;
            let level = map
                .get(KEY_HEADING_LEVEL)
                .and_then(|v| v.into_value().ok())
                .and_then(|v| {
                    if v.is_i64() {
                        v.into_i64().ok().map(|i| i as u8)
                    } else if v.is_double() {
                        v.into_double().ok().map(|d| d as u8)
                    } else {
                        None
                    }
                })
                .unwrap_or(1);
            let mut attr = NodeAttr::default();
            if let Some(jc) = get_str_from_map(map, KEY_HEADING_JC) {
                attr.kv.push(("jc".into(), jc));
            }
            if let Some(style) = get_str_from_map(map, KEY_HEADING_STYLE) {
                attr.kv.push(("style".into(), style));
            }
            Ok(Block::Heading(level, attr, inlines))
        }
        BLOCK_TYPE_CODE_BLOCK => {
            let text = map
                .get(KEY_CONTENT)
                .and_then(|v| v.into_container().ok())
                .and_then(|c| c.into_text().ok())
                .map(|t| t.to_string())
                .unwrap_or_default();
            Ok(Block::CodeBlock(NodeAttr::default(), text))
        }
        BLOCK_TYPE_HR => Ok(Block::HorizontalRule),
        BLOCK_TYPE_OPAQUE => Ok(super::opaque::read_opaque_block(map)),
        // Native table mapping (skeleton + live per-cell block lists). A legacy
        // stub written before this mapping has no skeleton and falls back to a
        // rule inside `read_table`.
        BLOCK_TYPE_TABLE => Ok(super::table::read_table(map)),
        // Legacy stubs: blocks written by bridge versions that predate the
        // opaque-snapshot scheme carry no content and cannot be recovered.
        BLOCK_TYPE_BULLET_LIST => {
            tracing::debug!("TODO/stub: loro bridge bullet list");
            Ok(Block::HorizontalRule)
        }
        BLOCK_TYPE_ORDERED_LIST => {
            tracing::debug!("TODO/stub: loro bridge ordered list");
            Ok(Block::HorizontalRule)
        }
        BLOCK_TYPE_FIGURE => {
            tracing::debug!("TODO/stub: loro bridge figure");
            Ok(Block::HorizontalRule)
        }
        _ => {
            let inlines = reconstruct_inlines(map)?;
            if inlines.is_empty() {
                Ok(Block::HorizontalRule)
            } else {
                Ok(Block::Plain(inlines))
            }
        }
    }
}

pub(super) fn reconstruct_blocks_from_list(list: &loro::LoroMovableList) -> Vec<Block> {
    let mut blocks = Vec::new();
    for i in 0..list.len() {
        if let Some(block_val) = list.get(i)
            && let Some(block_map) = block_val
                .into_container()
                .ok()
                .and_then(|c| c.into_map().ok())
            && let Ok(block) = map_loro_block(&block_map)
        {
            blocks.push(block);
        }
    }
    blocks
}

// ── PageLayout deserialization ────────────────────────────────────────────────

pub(super) fn reconstruct_page_layout(section_map: &LoroMap) -> PageLayout {
    let Some(layout_map) = section_map
        .get(KEY_LAYOUT)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
    else {
        return PageLayout::default();
    };

    let mut layout = PageLayout::default();

    // Page size
    if let Some(size_map) = layout_map
        .get(KEY_PAGE_SIZE)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
        && let (Some(w), Some(h)) = (
            get_f64_from_map(&size_map, "width"),
            get_f64_from_map(&size_map, "height"),
        )
    {
        layout.page_size = PageSize {
            width: Points::new(w),
            height: Points::new(h),
        };
    }

    // Margins
    if let Some(margins_map) = layout_map
        .get(KEY_MARGINS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
    {
        let mut m = PageMargins::default();
        if let Some(v) = get_f64_from_map(&margins_map, KEY_MARGIN_TOP) {
            m.top = Points::new(v);
        }
        if let Some(v) = get_f64_from_map(&margins_map, KEY_MARGIN_BOTTOM) {
            m.bottom = Points::new(v);
        }
        if let Some(v) = get_f64_from_map(&margins_map, KEY_MARGIN_LEFT) {
            m.left = Points::new(v);
        }
        if let Some(v) = get_f64_from_map(&margins_map, KEY_MARGIN_RIGHT) {
            m.right = Points::new(v);
        }
        if let Some(v) = get_f64_from_map(&margins_map, KEY_MARGIN_HEADER) {
            m.header = Points::new(v);
        }
        if let Some(v) = get_f64_from_map(&margins_map, KEY_MARGIN_FOOTER) {
            m.footer = Points::new(v);
        }
        if let Some(v) = get_f64_from_map(&margins_map, KEY_MARGIN_GUTTER) {
            m.gutter = Points::new(v);
        }
        layout.margins = m;
    }

    // Orientation
    if let Some(s) = get_str_from_map(&layout_map, KEY_ORIENTATION) {
        layout.orientation = if s == "Landscape" {
            PageOrientation::Landscape
        } else {
            PageOrientation::Portrait
        };
    }

    // Columns
    if let Some(cols_map) = layout_map
        .get(KEY_COLUMNS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
    {
        let count = get_i64_from_map(&cols_map, KEY_COL_COUNT).unwrap_or(1) as u8;
        let gap = get_f64_from_map(&cols_map, KEY_COL_GAP).unwrap_or(18.0);
        let separator = get_bool_from_map(&cols_map, KEY_COL_SEPARATOR).unwrap_or(false);
        layout.columns = Some(SectionColumns {
            count,
            gap: Points::new(gap),
            separator,
        });
    }

    // Header/footer slots
    layout.header =
        reconstruct_header_footer_slot(&layout_map, KEY_HEADER, HeaderFooterKind::Default);
    layout.footer =
        reconstruct_header_footer_slot(&layout_map, KEY_FOOTER, HeaderFooterKind::Default);
    layout.header_first =
        reconstruct_header_footer_slot(&layout_map, KEY_HEADER_FIRST, HeaderFooterKind::First);
    layout.footer_first =
        reconstruct_header_footer_slot(&layout_map, KEY_FOOTER_FIRST, HeaderFooterKind::First);
    layout.header_even =
        reconstruct_header_footer_slot(&layout_map, KEY_HEADER_EVEN, HeaderFooterKind::Even);
    layout.footer_even =
        reconstruct_header_footer_slot(&layout_map, KEY_FOOTER_EVEN, HeaderFooterKind::Even);

    layout
}

fn reconstruct_header_footer_slot(
    layout_map: &LoroMap,
    key: &str,
    kind: HeaderFooterKind,
) -> Option<HeaderFooter> {
    let list = layout_map
        .get(key)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_movable_list().ok())?;
    let blocks = reconstruct_blocks_from_list(&list);
    Some(HeaderFooter { kind, blocks })
}
