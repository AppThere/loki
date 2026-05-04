// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Serialization: Loki document model → Loro CRDT containers.

use loro::{LoroMap, LoroMovableList, LoroText};
use crate::content::block::Block;
use crate::layout::page::{PageLayout, PageOrientation};
use crate::layout::header_footer::HeaderFooter;
use crate::style::props::char_props::CharProps;
use crate::style::props::para_props::{ParaProps, ParagraphAlignment, Spacing, LineHeight};
use crate::loro_schema::*;
use super::BridgeError;
use super::inlines::map_inlines;

// ── Block serialization ───────────────────────────────────────────────────────

pub(super) fn map_block(block: &Block, index: usize, map: &LoroMap) -> Result<(), BridgeError> {
    match block {
        Block::Para(inlines) => {
            map.insert(KEY_TYPE, BLOCK_TYPE_PARA)?;
            let content = map.insert_container(KEY_CONTENT, LoroText::new())?;
            map_inlines(inlines, &content)?;
        }
        Block::StyledPara(para) => {
            map.insert(KEY_TYPE, BLOCK_TYPE_STYLED_PARA)?;
            if let Some(style_id) = &para.style_id {
                map.insert("style_id", style_id.as_str())?;
            }
            if let Some(para_props) = &para.direct_para_props {
                let props_map = map.insert_container(KEY_PARA_PROPS, LoroMap::new())?;
                map_para_props(para_props, &props_map)?;
            }
            if let Some(char_props) = &para.direct_char_props {
                let props_map = map.insert_container(KEY_DIRECT_CHAR_PROPS, LoroMap::new())?;
                map_char_props_to_map(char_props, &props_map)?;
            }
            let content = map.insert_container(KEY_CONTENT, LoroText::new())?;
            map_inlines(&para.inlines, &content)?;
        }
        Block::Heading(level, _, inlines) => {
            map.insert(KEY_TYPE, BLOCK_TYPE_HEADING)?;
            map.insert("level", *level as i32)?;
            let content = map.insert_container(KEY_CONTENT, LoroText::new())?;
            map_inlines(inlines, &content)?;
        }
        Block::CodeBlock(_, content_str) => {
            map.insert(KEY_TYPE, BLOCK_TYPE_CODE_BLOCK)?;
            let content = map.insert_container(KEY_CONTENT, LoroText::new())?;
            content.insert(0, content_str)?;
        }
        Block::HorizontalRule => {
            map.insert(KEY_TYPE, BLOCK_TYPE_HR)?;
        }
        Block::Plain(inlines) => {
            map.insert(KEY_TYPE, BLOCK_TYPE_PARA)?;
            let content = map.insert_container(KEY_CONTENT, LoroText::new())?;
            map_inlines(inlines, &content)?;
        }
        Block::BulletList(_) => {
            tracing::debug!("stub: bullet list at index {}", index);
        }
        Block::OrderedList(_, _) => {
            tracing::debug!("stub: ordered list at index {}", index);
        }
        Block::Table(_) => {
            tracing::debug!("stub: table at index {}", index);
        }
        Block::Figure(_, _, _) => {
            tracing::debug!("stub: figure at index {}", index);
        }
        Block::BlockQuote(_) => {
            tracing::debug!("stub: block quote at index {}", index);
        }
        _ => {
            tracing::debug!("stub: other block variant at index {}", index);
        }
    }
    Ok(())
}

pub(super) fn map_blocks_to_list(blocks: &[Block], list: &LoroMovableList) -> Result<(), BridgeError> {
    for (i, block) in blocks.iter().enumerate() {
        let block_map = list.insert_container(i, LoroMap::new())?;
        map_block(block, i, &block_map)?;
    }
    Ok(())
}

// ── PageLayout serialization ──────────────────────────────────────────────────

pub(super) fn map_page_layout(layout: &PageLayout, section_map: &LoroMap) -> Result<(), BridgeError> {
    let layout_map = section_map.insert_container(KEY_LAYOUT, LoroMap::new())?;

    // Page size
    let size_map = layout_map.insert_container(KEY_PAGE_SIZE, LoroMap::new())?;
    size_map.insert("width", layout.page_size.width.value())?;
    size_map.insert("height", layout.page_size.height.value())?;

    // Margins
    let margins_map = layout_map.insert_container(KEY_MARGINS, LoroMap::new())?;
    margins_map.insert(KEY_MARGIN_TOP, layout.margins.top.value())?;
    margins_map.insert(KEY_MARGIN_BOTTOM, layout.margins.bottom.value())?;
    margins_map.insert(KEY_MARGIN_LEFT, layout.margins.left.value())?;
    margins_map.insert(KEY_MARGIN_RIGHT, layout.margins.right.value())?;
    margins_map.insert(KEY_MARGIN_HEADER, layout.margins.header.value())?;
    margins_map.insert(KEY_MARGIN_FOOTER, layout.margins.footer.value())?;
    margins_map.insert(KEY_MARGIN_GUTTER, layout.margins.gutter.value())?;

    // Orientation
    let orientation = match layout.orientation {
        PageOrientation::Portrait => "Portrait",
        PageOrientation::Landscape => "Landscape",
    };
    layout_map.insert(KEY_ORIENTATION, orientation)?;

    // Columns (optional)
    if let Some(cols) = &layout.columns {
        let cols_map = layout_map.insert_container(KEY_COLUMNS, LoroMap::new())?;
        cols_map.insert(KEY_COL_COUNT, i64::from(cols.count))?;
        cols_map.insert(KEY_COL_GAP, cols.gap.value())?;
        cols_map.insert(KEY_COL_SEPARATOR, cols.separator)?;
    }

    // Header/footer slots
    map_header_footer_slot(&layout.header, KEY_HEADER, &layout_map)?;
    map_header_footer_slot(&layout.footer, KEY_FOOTER, &layout_map)?;
    map_header_footer_slot(&layout.header_first, KEY_HEADER_FIRST, &layout_map)?;
    map_header_footer_slot(&layout.footer_first, KEY_FOOTER_FIRST, &layout_map)?;
    map_header_footer_slot(&layout.header_even, KEY_HEADER_EVEN, &layout_map)?;
    map_header_footer_slot(&layout.footer_even, KEY_FOOTER_EVEN, &layout_map)?;

    Ok(())
}

fn map_header_footer_slot(
    hf: &Option<HeaderFooter>,
    key: &str,
    layout_map: &LoroMap,
) -> Result<(), BridgeError> {
    if let Some(hf) = hf {
        let list = layout_map.insert_container(key, LoroMovableList::new())?;
        map_blocks_to_list(&hf.blocks, &list)?;
    }
    Ok(())
}

// ── ParaProps serialization ───────────────────────────────────────────────────

pub(super) fn map_para_props(props: &ParaProps, map: &LoroMap) -> Result<(), BridgeError> {
    if let Some(v) = &props.alignment { map.insert(PROP_ALIGNMENT, encode_alignment(v))?; }
    if let Some(v) = &props.indent_start { map.insert(PROP_INDENT_LEFT, v.value())?; }
    if let Some(v) = &props.indent_end { map.insert(PROP_INDENT_RIGHT, v.value())?; }
    if let Some(v) = &props.indent_first_line { map.insert(PROP_INDENT_FIRST_LINE, v.value())?; }
    if let Some(v) = &props.indent_hanging { map.insert(PROP_INDENT_HANGING, v.value())?; }
    if let Some(v) = props.keep_together { map.insert(PROP_KEEP_TOGETHER, v)?; }
    if let Some(v) = props.keep_with_next { map.insert(PROP_KEEP_WITH_NEXT, v)?; }
    if let Some(v) = props.page_break_after { map.insert(PROP_PAGE_BREAK_AFTER, v)?; }
    if let Some(v) = props.bidi { map.insert(PROP_BIDI, v)?; }
    if let Some(v) = props.widow_control { map.insert(PROP_WIDOW_CONTROL, i32::from(v))?; }
    if let Some(v) = props.list_level { map.insert(PROP_LIST_LEVEL, i32::from(v))?; }
    if let Some(v) = &props.space_before { map.insert(PROP_SPACE_BEFORE_PT, encode_spacing(v))?; }
    if let Some(v) = &props.space_after { map.insert(PROP_SPACE_AFTER_PT, encode_spacing(v))?; }
    if let Some(v) = &props.line_height { map.insert(PROP_LINE_HEIGHT, encode_line_height(v))?; }
    if let Some(v) = &props.list_id { map.insert(PROP_LIST_ID, v.as_str())?; }
    if let Some(v) = &props.tab_stops { map.insert(PROP_TAB_STOPS, format!("{:?}", v))?; }
    if let Some(v) = &props.background_color { map.insert("background_color", format!("{:?}", v))?; }
    Ok(())
}

// ── CharProps map serialization ───────────────────────────────────────────────

pub(super) fn map_char_props_to_map(props: &CharProps, map: &LoroMap) -> Result<(), BridgeError> {
    if let Some(v) = props.bold { map.insert("bold", v)?; }
    if let Some(v) = props.italic { map.insert("italic", v)?; }
    if let Some(v) = props.outline { map.insert("outline", v)?; }
    if let Some(v) = props.shadow { map.insert("shadow", v)?; }
    if let Some(v) = props.small_caps { map.insert("small_caps", v)?; }
    if let Some(v) = props.all_caps { map.insert("all_caps", v)?; }
    if let Some(v) = props.kerning { map.insert("kerning", v)?; }
    if let Some(v) = &props.font_name { map.insert("font_name", v.as_str())?; }
    if let Some(v) = &props.font_name_complex { map.insert("font_name_complex", v.as_str())?; }
    if let Some(v) = &props.font_name_east_asian { map.insert("font_name_east_asian", v.as_str())?; }
    if let Some(v) = &props.font_size { map.insert("font_size", v.value())?; }
    if let Some(v) = &props.font_size_complex { map.insert("font_size_complex", v.value())?; }
    if let Some(v) = props.scale { map.insert("scale", f64::from(v))?; }
    if let Some(v) = &props.letter_spacing { map.insert("letter_spacing", v.value())?; }
    if let Some(v) = &props.word_spacing { map.insert("word_spacing", v.value())?; }
    if let Some(v) = &props.underline { map.insert("underline", format!("{:?}", v))?; }
    if let Some(v) = &props.strikethrough { map.insert("strikethrough", format!("{:?}", v))?; }
    if let Some(v) = &props.vertical_align { map.insert("vertical_align", format!("{:?}", v))?; }
    if let Some(v) = &props.hyperlink { map.insert("hyperlink", v.as_str())?; }
    if let Some(hex) = props.color.as_ref().and_then(|c| c.to_hex()) {
        map.insert("color", hex)?;
    }
    if let Some(hex) = props.background_color.as_ref().and_then(|c| c.to_hex()) {
        map.insert("background_color", hex)?;
    }
    if let Some(v) = &props.highlight_color { map.insert("highlight_color", format!("{v:?}"))?; }
    Ok(())
}

// ── Encode helpers ────────────────────────────────────────────────────────────

pub(super) fn encode_alignment(a: &ParagraphAlignment) -> &'static str {
    match a {
        ParagraphAlignment::Left => "Left",
        ParagraphAlignment::Right => "Right",
        ParagraphAlignment::Center => "Center",
        ParagraphAlignment::Justify => "Justify",
        ParagraphAlignment::Distribute => "Distribute",
    }
}

pub(super) fn encode_spacing(s: &Spacing) -> String {
    match s {
        Spacing::Exact(pts) => format!("Exact:{}", pts.value()),
        Spacing::Percent(pct) => format!("Percent:{}", pct),
    }
}

pub(super) fn encode_line_height(lh: &LineHeight) -> String {
    match lh {
        LineHeight::Exact(pts) => format!("Exact:{}", pts.value()),
        LineHeight::AtLeast(pts) => format!("AtLeast:{}", pts.value()),
        LineHeight::Multiple(m) => format!("Multiple:{}", m),
    }
}
