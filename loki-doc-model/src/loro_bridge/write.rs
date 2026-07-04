// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Serialization: Loki document model → Loro CRDT containers.

use super::BridgeError;
use super::color_codec::encode_document_color;
use super::decode::{
    encode_alignment, encode_border, encode_line_height, encode_spacing, encode_tab_stops,
};
use super::inlines::map_inlines;
use crate::content::block::Block;
use crate::loro_schema::*;
use crate::style::props::char_props::CharProps;
use crate::style::props::para_props::ParaProps;
use loro::{LoroMap, LoroMovableList, LoroText};

// ── Block serialization ───────────────────────────────────────────────────────

pub(crate) fn map_block(block: &Block, map: &LoroMap) -> Result<(), BridgeError> {
    // Tables and container blocks (lists, quotes, divs, figures) have native
    // mappings: structural metadata plus live nested block lists (see
    // `table.rs` / `containers.rs`). Without `serde` there is no metadata
    // format, so they take the opaque path below instead.
    #[cfg(feature = "serde")]
    match block {
        Block::Table(table) => return super::table::write_table(table, map),
        Block::BulletList(_)
        | Block::OrderedList(_, _)
        | Block::BlockQuote(_)
        | Block::Div(_, _)
        | Block::Figure(_, _, _) => return super::containers::write_container(block, map),
        _ => {}
    }
    // Blocks (or paragraphs whose inline content) the flat text schema cannot
    // represent are preserved verbatim as opaque JSON snapshots so that a
    // document_to_loro → loro_to_document round-trip is lossless. See
    // `opaque.rs` for the rationale.
    if !super::opaque::block_round_trips_as_text(block) {
        return super::opaque::write_opaque_block(block, map);
    }
    match block {
        Block::Para(inlines) => {
            map.insert(KEY_TYPE, BLOCK_TYPE_PARA)?;
            let content = map.insert_container(KEY_CONTENT, LoroText::new())?;
            map_inlines(inlines, &content, map)?;
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
            map_inlines(&para.inlines, &content, map)?;
        }
        Block::Heading(level, attr, inlines) => {
            map.insert(KEY_TYPE, BLOCK_TYPE_HEADING)?;
            map.insert(KEY_HEADING_LEVEL, *level as i32)?;
            // Persist NodeAttr keys so alignment and style name survive
            // loro_to_document round-trips after mutations.
            if let Some((_, jc)) = attr.kv.iter().find(|(k, _)| k == "jc") {
                map.insert(KEY_HEADING_JC, jc.as_str())?;
            }
            if let Some((_, style)) = attr.kv.iter().find(|(k, _)| k == "style") {
                map.insert(KEY_HEADING_STYLE, style.as_str())?;
            }
            let content = map.insert_container(KEY_CONTENT, LoroText::new())?;
            map_inlines(inlines, &content, map)?;
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
            map_inlines(inlines, &content, map)?;
        }
        // All structurally unsupported variants (lists, tables, figures,
        // blockquotes, …) take the opaque-snapshot early return above; this
        // arm only guards future #[non_exhaustive] additions.
        _ => {
            super::opaque::write_opaque_block(block, map)?;
        }
    }
    Ok(())
}

pub(super) fn map_blocks_to_list(
    blocks: &[Block],
    list: &LoroMovableList,
) -> Result<(), BridgeError> {
    for (i, block) in blocks.iter().enumerate() {
        let block_map = list.insert_container(i, LoroMap::new())?;
        map_block(block, &block_map)?;
    }
    Ok(())
}

// ── ParaProps serialization ───────────────────────────────────────────────────

pub(super) fn map_para_props(props: &ParaProps, map: &LoroMap) -> Result<(), BridgeError> {
    if let Some(v) = &props.alignment {
        map.insert(PROP_ALIGNMENT, encode_alignment(v))?;
    }
    if let Some(v) = &props.indent_start {
        map.insert(PROP_INDENT_LEFT, v.value())?;
    }
    if let Some(v) = &props.indent_end {
        map.insert(PROP_INDENT_RIGHT, v.value())?;
    }
    if let Some(v) = &props.indent_first_line {
        map.insert(PROP_INDENT_FIRST_LINE, v.value())?;
    }
    if let Some(v) = &props.indent_hanging {
        map.insert(PROP_INDENT_HANGING, v.value())?;
    }
    if let Some(v) = props.keep_together {
        map.insert(PROP_KEEP_TOGETHER, v)?;
    }
    if let Some(v) = props.keep_with_next {
        map.insert(PROP_KEEP_WITH_NEXT, v)?;
    }
    if let Some(v) = props.page_break_after {
        map.insert(PROP_PAGE_BREAK_AFTER, v)?;
    }
    if let Some(v) = props.page_break_before {
        map.insert(PROP_PAGE_BREAK_BEFORE, v)?;
    }
    if let Some(v) = props.bidi {
        map.insert(PROP_BIDI, v)?;
    }
    if let Some(v) = props.widow_control {
        map.insert(PROP_WIDOW_CONTROL, i32::from(v))?;
    }
    if let Some(v) = props.orphan_control {
        map.insert(PROP_ORPHAN_CONTROL, i32::from(v))?;
    }
    if let Some(v) = props.outline_level {
        map.insert(PROP_OUTLINE_LEVEL, i32::from(v))?;
    }
    if let Some(v) = props.list_level {
        map.insert(PROP_LIST_LEVEL, i32::from(v))?;
    }
    if let Some(v) = &props.space_before {
        map.insert(PROP_SPACE_BEFORE_PT, encode_spacing(v))?;
    }
    if let Some(v) = &props.space_after {
        map.insert(PROP_SPACE_AFTER_PT, encode_spacing(v))?;
    }
    if let Some(v) = &props.line_height {
        map.insert(PROP_LINE_HEIGHT, encode_line_height(v))?;
    }
    if let Some(v) = &props.list_id {
        map.insert(PROP_LIST_ID, v.as_str())?;
    }
    if let Some(v) = &props.border_top {
        map.insert(PROP_BORDER_TOP, encode_border(v))?;
    }
    if let Some(v) = &props.border_bottom {
        map.insert(PROP_BORDER_BOTTOM, encode_border(v))?;
    }
    if let Some(v) = &props.border_left {
        map.insert(PROP_BORDER_LEFT, encode_border(v))?;
    }
    if let Some(v) = &props.border_right {
        map.insert(PROP_BORDER_RIGHT, encode_border(v))?;
    }
    if let Some(v) = &props.border_between {
        map.insert(PROP_BORDER_BETWEEN, encode_border(v))?;
    }
    if let Some(v) = &props.padding_top {
        map.insert(PROP_PADDING_TOP, v.value())?;
    }
    if let Some(v) = &props.padding_bottom {
        map.insert(PROP_PADDING_BOTTOM, v.value())?;
    }
    if let Some(v) = &props.padding_left {
        map.insert(PROP_PADDING_LEFT, v.value())?;
    }
    if let Some(v) = &props.padding_right {
        map.insert(PROP_PADDING_RIGHT, v.value())?;
    }
    if let Some(v) = &props.tab_stops {
        map.insert(PROP_TAB_STOPS, encode_tab_stops(v))?;
    }
    if let Some(v) = &props.background_color {
        map.insert(PROP_BACKGROUND_COLOR, encode_document_color(v))?;
    }
    Ok(())
}

// ── CharProps map serialization ───────────────────────────────────────────────

pub(super) fn map_char_props_to_map(props: &CharProps, map: &LoroMap) -> Result<(), BridgeError> {
    if let Some(v) = props.bold {
        map.insert("bold", v)?;
    }
    if let Some(v) = props.italic {
        map.insert("italic", v)?;
    }
    if let Some(v) = props.outline {
        map.insert("outline", v)?;
    }
    if let Some(v) = props.shadow {
        map.insert("shadow", v)?;
    }
    if let Some(v) = props.small_caps {
        map.insert("small_caps", v)?;
    }
    if let Some(v) = props.all_caps {
        map.insert("all_caps", v)?;
    }
    if let Some(v) = props.kerning {
        map.insert("kerning", v)?;
    }
    if let Some(v) = &props.font_name {
        map.insert("font_name", v.as_str())?;
    }
    if let Some(v) = &props.font_name_complex {
        map.insert("font_name_complex", v.as_str())?;
    }
    if let Some(v) = &props.font_name_east_asian {
        map.insert("font_name_east_asian", v.as_str())?;
    }
    if let Some(v) = &props.font_size {
        map.insert("font_size", v.value())?;
    }
    if let Some(v) = &props.font_size_complex {
        map.insert("font_size_complex", v.value())?;
    }
    if let Some(v) = props.scale {
        map.insert("scale", f64::from(v))?;
    }
    if let Some(v) = &props.letter_spacing {
        map.insert("letter_spacing", v.value())?;
    }
    if let Some(v) = &props.word_spacing {
        map.insert("word_spacing", v.value())?;
    }
    if let Some(v) = &props.underline {
        map.insert("underline", format!("{:?}", v))?;
    }
    if let Some(v) = &props.strikethrough {
        map.insert("strikethrough", format!("{:?}", v))?;
    }
    if let Some(v) = &props.vertical_align {
        map.insert("vertical_align", format!("{:?}", v))?;
    }
    if let Some(v) = &props.hyperlink {
        map.insert("hyperlink", v.as_str())?;
    }
    if let Some(hex) = props.color.as_ref().and_then(|c| c.to_hex()) {
        map.insert("color", hex)?;
    }
    if let Some(hex) = props.background_color.as_ref().and_then(|c| c.to_hex()) {
        map.insert("background_color", hex)?;
    }
    if let Some(v) = &props.highlight_color {
        map.insert("highlight_color", format!("{v:?}"))?;
    }
    if let Some(v) = &props.language {
        map.insert("language", v.as_str())?;
    }
    if let Some(v) = &props.language_complex {
        map.insert("language_complex", v.as_str())?;
    }
    if let Some(v) = &props.language_east_asian {
        map.insert("language_east_asian", v.as_str())?;
    }
    Ok(())
}
