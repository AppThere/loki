// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Deserialization: Loro CRDT containers → Loki document model.

use loro::LoroMap;
use loki_primitives::units::Points;
use crate::content::block::Block;
use crate::content::attr::NodeAttr;
use crate::layout::page::{PageLayout, PageSize, PageMargins, PageOrientation, SectionColumns};
use crate::layout::header_footer::{HeaderFooter, HeaderFooterKind};
use crate::style::props::char_props::CharProps;
use crate::style::props::para_props::{ParaProps, ParagraphAlignment, Spacing, LineHeight};
use crate::style::list_style::ListId;
use crate::loro_schema::*;
use super::BridgeError;
use loki_primitives::color::DocumentColor;
use super::inlines::{reconstruct_inlines, decode_highlight_color, decode_underline, decode_strikethrough, decode_vertical_align};

// ── Block deserialization ─────────────────────────────────────────────────────

pub(super) fn map_loro_block(map: &LoroMap) -> Result<Block, BridgeError> {
    let block_type = map
        .get(KEY_TYPE)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())
        .unwrap_or_default();

    match block_type.as_str() {
        BLOCK_TYPE_PARA => {
            Ok(Block::Para(reconstruct_inlines(map)?))
        }
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
                .get("level")
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
            Ok(Block::Heading(level, NodeAttr::default(), inlines))
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
        BLOCK_TYPE_TABLE => {
            tracing::debug!("TODO/stub: loro bridge table");
            Ok(Block::HorizontalRule)
        }
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
            && let Some(block_map) = block_val.into_container().ok().and_then(|c| c.into_map().ok())
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
        layout.page_size = PageSize { width: Points::new(w), height: Points::new(h) };
    }

    // Margins
    if let Some(margins_map) = layout_map
        .get(KEY_MARGINS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())
    {
        let mut m = PageMargins::default();
        if let Some(v) = get_f64_from_map(&margins_map, KEY_MARGIN_TOP) { m.top = Points::new(v); }
        if let Some(v) = get_f64_from_map(&margins_map, KEY_MARGIN_BOTTOM) { m.bottom = Points::new(v); }
        if let Some(v) = get_f64_from_map(&margins_map, KEY_MARGIN_LEFT) { m.left = Points::new(v); }
        if let Some(v) = get_f64_from_map(&margins_map, KEY_MARGIN_RIGHT) { m.right = Points::new(v); }
        if let Some(v) = get_f64_from_map(&margins_map, KEY_MARGIN_HEADER) { m.header = Points::new(v); }
        if let Some(v) = get_f64_from_map(&margins_map, KEY_MARGIN_FOOTER) { m.footer = Points::new(v); }
        if let Some(v) = get_f64_from_map(&margins_map, KEY_MARGIN_GUTTER) { m.gutter = Points::new(v); }
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
        layout.columns = Some(SectionColumns { count, gap: Points::new(gap), separator });
    }

    // Header/footer slots
    layout.header = reconstruct_header_footer_slot(&layout_map, KEY_HEADER, HeaderFooterKind::Default);
    layout.footer = reconstruct_header_footer_slot(&layout_map, KEY_FOOTER, HeaderFooterKind::Default);
    layout.header_first = reconstruct_header_footer_slot(&layout_map, KEY_HEADER_FIRST, HeaderFooterKind::First);
    layout.footer_first = reconstruct_header_footer_slot(&layout_map, KEY_FOOTER_FIRST, HeaderFooterKind::First);
    layout.header_even = reconstruct_header_footer_slot(&layout_map, KEY_HEADER_EVEN, HeaderFooterKind::Even);
    layout.footer_even = reconstruct_header_footer_slot(&layout_map, KEY_FOOTER_EVEN, HeaderFooterKind::Even);

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

// ── ParaProps reconstruction ──────────────────────────────────────────────────

pub(super) fn reconstruct_para_props(block_map: &LoroMap) -> Option<ParaProps> {
    let props_map = block_map
        .get(KEY_PARA_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())?;

    let mut props = ParaProps::default();
    let mut any = false;

    if let Some(s) = get_str_from_map(&props_map, PROP_ALIGNMENT)
        && let Some(a) = decode_alignment(&s) { props.alignment = Some(a); any = true; }
    if let Some(v) = get_f64_from_map(&props_map, PROP_INDENT_LEFT) { props.indent_start = Some(Points::new(v)); any = true; }
    if let Some(v) = get_f64_from_map(&props_map, PROP_INDENT_RIGHT) { props.indent_end = Some(Points::new(v)); any = true; }
    if let Some(v) = get_f64_from_map(&props_map, PROP_INDENT_FIRST_LINE) { props.indent_first_line = Some(Points::new(v)); any = true; }
    if let Some(v) = get_f64_from_map(&props_map, PROP_INDENT_HANGING) { props.indent_hanging = Some(Points::new(v)); any = true; }
    if let Some(v) = get_bool_from_map(&props_map, PROP_KEEP_TOGETHER) { props.keep_together = Some(v); any = true; }
    if let Some(v) = get_bool_from_map(&props_map, PROP_KEEP_WITH_NEXT) { props.keep_with_next = Some(v); any = true; }
    if let Some(v) = get_bool_from_map(&props_map, PROP_PAGE_BREAK_AFTER) { props.page_break_after = Some(v); any = true; }
    if let Some(v) = get_bool_from_map(&props_map, PROP_BIDI) { props.bidi = Some(v); any = true; }
    if let Some(v) = get_i64_from_map(&props_map, PROP_WIDOW_CONTROL) { props.widow_control = Some(v as u8); any = true; }
    if let Some(v) = get_i64_from_map(&props_map, PROP_LIST_LEVEL) { props.list_level = Some(v as u8); any = true; }
    if let Some(s) = get_str_from_map(&props_map, PROP_SPACE_BEFORE_PT)
        && let Some(sp) = decode_spacing(&s) { props.space_before = Some(sp); any = true; }
    if let Some(s) = get_str_from_map(&props_map, PROP_SPACE_AFTER_PT)
        && let Some(sp) = decode_spacing(&s) { props.space_after = Some(sp); any = true; }
    if let Some(s) = get_str_from_map(&props_map, PROP_LINE_HEIGHT)
        && let Some(lh) = decode_line_height(&s) { props.line_height = Some(lh); any = true; }
    if let Some(s) = get_str_from_map(&props_map, PROP_LIST_ID) {
        props.list_id = Some(ListId::new(s)); any = true;
    }
    // tab_stops and background_color: complex Debug-format strings, deferred

    if any { Some(props) } else { None }
}

// ── CharProps map reconstruction ──────────────────────────────────────────────

pub(super) fn reconstruct_char_props_from_map(block_map: &LoroMap) -> Option<CharProps> {
    let props_map = block_map
        .get(KEY_DIRECT_CHAR_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())?;

    let mut props = CharProps::default();
    let mut any = false;

    macro_rules! set_bool {
        ($field:ident, $key:literal) => {
            if let Some(v) = get_bool_from_map(&props_map, $key) { props.$field = Some(v); any = true; }
        };
    }
    set_bool!(bold, "bold");
    set_bool!(italic, "italic");
    set_bool!(outline, "outline");
    set_bool!(shadow, "shadow");
    set_bool!(small_caps, "small_caps");
    set_bool!(all_caps, "all_caps");
    set_bool!(kerning, "kerning");

    if let Some(s) = get_str_from_map(&props_map, "font_name") { props.font_name = Some(s); any = true; }
    if let Some(s) = get_str_from_map(&props_map, "font_name_complex") { props.font_name_complex = Some(s); any = true; }
    if let Some(s) = get_str_from_map(&props_map, "font_name_east_asian") { props.font_name_east_asian = Some(s); any = true; }
    if let Some(v) = get_f64_from_map(&props_map, "font_size") { props.font_size = Some(Points::new(v)); any = true; }
    if let Some(v) = get_f64_from_map(&props_map, "font_size_complex") { props.font_size_complex = Some(Points::new(v)); any = true; }
    if let Some(v) = get_f64_from_map(&props_map, "scale") { props.scale = Some(v as f32); any = true; }
    if let Some(v) = get_f64_from_map(&props_map, "letter_spacing") { props.letter_spacing = Some(Points::new(v)); any = true; }
    if let Some(v) = get_f64_from_map(&props_map, "word_spacing") { props.word_spacing = Some(Points::new(v)); any = true; }
    if let Some(s) = get_str_from_map(&props_map, "underline")
        && let Some(u) = decode_underline(&s) { props.underline = Some(u); any = true; }
    if let Some(s) = get_str_from_map(&props_map, "strikethrough")
        && let Some(st) = decode_strikethrough(&s) { props.strikethrough = Some(st); any = true; }
    if let Some(s) = get_str_from_map(&props_map, "vertical_align")
        && let Some(va) = decode_vertical_align(&s) { props.vertical_align = Some(va); any = true; }
    if let Some(s) = get_str_from_map(&props_map, "hyperlink") { props.hyperlink = Some(s); any = true; }
    if let Some(s) = get_str_from_map(&props_map, "color")
        && let Ok(c) = DocumentColor::from_hex(&s) { props.color = Some(c); any = true; }
    if let Some(s) = get_str_from_map(&props_map, "background_color")
        && let Ok(c) = DocumentColor::from_hex(&s) { props.background_color = Some(c); any = true; }
    if let Some(s) = get_str_from_map(&props_map, "highlight_color")
        && let Some(h) = decode_highlight_color(&s) { props.highlight_color = Some(h); any = true; }

    if any { Some(props) } else { None }
}

// ── Loro map value accessors ──────────────────────────────────────────────────

pub(super) fn get_str_from_map(map: &LoroMap, key: &str) -> Option<String> {
    map.get(key)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_string().ok())
        .map(|s| s.to_string())
}

pub(super) fn get_f64_from_map(map: &LoroMap, key: &str) -> Option<f64> {
    map.get(key)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_double().ok())
}

pub(super) fn get_bool_from_map(map: &LoroMap, key: &str) -> Option<bool> {
    map.get(key)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_bool().ok())
}

pub(super) fn get_i64_from_map(map: &LoroMap, key: &str) -> Option<i64> {
    map.get(key)
        .and_then(|v| v.into_value().ok())
        .and_then(|v| v.into_i64().ok())
}

// ── Enum decode helpers ───────────────────────────────────────────────────────

fn decode_alignment(s: &str) -> Option<ParagraphAlignment> {
    match s {
        "Left" => Some(ParagraphAlignment::Left),
        "Right" => Some(ParagraphAlignment::Right),
        "Center" => Some(ParagraphAlignment::Center),
        "Justify" => Some(ParagraphAlignment::Justify),
        "Distribute" => Some(ParagraphAlignment::Distribute),
        _ => None,
    }
}

fn decode_spacing(s: &str) -> Option<Spacing> {
    if let Some(rest) = s.strip_prefix("Exact:") {
        rest.parse::<f64>().ok().map(|v| Spacing::Exact(Points::new(v)))
    } else if let Some(rest) = s.strip_prefix("Percent:") {
        rest.parse::<f32>().ok().map(Spacing::Percent)
    } else {
        None
    }
}

fn decode_line_height(s: &str) -> Option<LineHeight> {
    if let Some(rest) = s.strip_prefix("Exact:") {
        rest.parse::<f64>().ok().map(|v| LineHeight::Exact(Points::new(v)))
    } else if let Some(rest) = s.strip_prefix("AtLeast:") {
        rest.parse::<f64>().ok().map(|v| LineHeight::AtLeast(Points::new(v)))
    } else if let Some(rest) = s.strip_prefix("Multiple:") {
        rest.parse::<f32>().ok().map(LineHeight::Multiple)
    } else {
        None
    }
}
