// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ParaProps and CharProps reconstruction from Loro CRDT maps.
//!
//! Split from `read.rs` to keep individual files under the 300-line ceiling.

use super::decode::{
    decode_alignment, decode_border, decode_highlight_color, decode_line_height, decode_spacing,
    decode_strikethrough, decode_underline, decode_vertical_align,
};
use crate::loro_schema::*;
use crate::meta::language::LanguageTag;
use crate::style::list_style::ListId;
use crate::style::props::char_props::CharProps;
use crate::style::props::para_props::ParaProps;
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;
use loro::LoroMap;

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

// ── ParaProps reconstruction ──────────────────────────────────────────────────

pub(super) fn reconstruct_para_props(block_map: &LoroMap) -> Option<ParaProps> {
    let props_map = block_map
        .get(KEY_PARA_PROPS)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_map().ok())?;

    let mut props = ParaProps::default();
    let mut any = false;

    if let Some(s) = get_str_from_map(&props_map, PROP_ALIGNMENT)
        && let Some(a) = decode_alignment(&s)
    {
        props.alignment = Some(a);
        any = true;
    }
    if let Some(v) = get_f64_from_map(&props_map, PROP_INDENT_LEFT) {
        props.indent_start = Some(Points::new(v));
        any = true;
    }
    if let Some(v) = get_f64_from_map(&props_map, PROP_INDENT_RIGHT) {
        props.indent_end = Some(Points::new(v));
        any = true;
    }
    if let Some(v) = get_f64_from_map(&props_map, PROP_INDENT_FIRST_LINE) {
        props.indent_first_line = Some(Points::new(v));
        any = true;
    }
    if let Some(v) = get_f64_from_map(&props_map, PROP_INDENT_HANGING) {
        props.indent_hanging = Some(Points::new(v));
        any = true;
    }
    if let Some(v) = get_bool_from_map(&props_map, PROP_KEEP_TOGETHER) {
        props.keep_together = Some(v);
        any = true;
    }
    if let Some(v) = get_bool_from_map(&props_map, PROP_KEEP_WITH_NEXT) {
        props.keep_with_next = Some(v);
        any = true;
    }
    if let Some(v) = get_bool_from_map(&props_map, PROP_PAGE_BREAK_AFTER) {
        props.page_break_after = Some(v);
        any = true;
    }
    if let Some(v) = get_bool_from_map(&props_map, PROP_PAGE_BREAK_BEFORE) {
        props.page_break_before = Some(v);
        any = true;
    }
    if let Some(v) = get_bool_from_map(&props_map, PROP_BIDI) {
        props.bidi = Some(v);
        any = true;
    }
    if let Some(v) = get_i64_from_map(&props_map, PROP_WIDOW_CONTROL) {
        props.widow_control = Some(v as u8);
        any = true;
    }
    if let Some(v) = get_i64_from_map(&props_map, PROP_ORPHAN_CONTROL) {
        props.orphan_control = Some(v as u8);
        any = true;
    }
    if let Some(v) = get_i64_from_map(&props_map, PROP_OUTLINE_LEVEL) {
        props.outline_level = Some(v as u8);
        any = true;
    }
    if let Some(v) = get_i64_from_map(&props_map, PROP_LIST_LEVEL) {
        props.list_level = Some(v as u8);
        any = true;
    }
    if let Some(s) = get_str_from_map(&props_map, PROP_SPACE_BEFORE_PT)
        && let Some(sp) = decode_spacing(&s)
    {
        props.space_before = Some(sp);
        any = true;
    }
    if let Some(s) = get_str_from_map(&props_map, PROP_SPACE_AFTER_PT)
        && let Some(sp) = decode_spacing(&s)
    {
        props.space_after = Some(sp);
        any = true;
    }
    if let Some(s) = get_str_from_map(&props_map, PROP_LINE_HEIGHT)
        && let Some(lh) = decode_line_height(&s)
    {
        props.line_height = Some(lh);
        any = true;
    }
    if let Some(s) = get_str_from_map(&props_map, PROP_LIST_ID) {
        props.list_id = Some(ListId::new(s));
        any = true;
    }

    // Paragraph borders
    macro_rules! read_border {
        ($field:ident, $key:expr) => {
            if let Some(s) = get_str_from_map(&props_map, $key)
                && let Some(b) = decode_border(&s)
            {
                props.$field = Some(b);
                any = true;
            }
        };
    }
    read_border!(border_top, PROP_BORDER_TOP);
    read_border!(border_bottom, PROP_BORDER_BOTTOM);
    read_border!(border_left, PROP_BORDER_LEFT);
    read_border!(border_right, PROP_BORDER_RIGHT);
    read_border!(border_between, PROP_BORDER_BETWEEN);

    // Padding
    macro_rules! read_pt {
        ($field:ident, $key:expr) => {
            if let Some(v) = get_f64_from_map(&props_map, $key) {
                props.$field = Some(Points::new(v));
                any = true;
            }
        };
    }
    read_pt!(padding_top, PROP_PADDING_TOP);
    read_pt!(padding_bottom, PROP_PADDING_BOTTOM);
    read_pt!(padding_left, PROP_PADDING_LEFT);
    read_pt!(padding_right, PROP_PADDING_RIGHT);

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
            if let Some(v) = get_bool_from_map(&props_map, $key) {
                props.$field = Some(v);
                any = true;
            }
        };
    }
    set_bool!(bold, "bold");
    set_bool!(italic, "italic");
    set_bool!(outline, "outline");
    set_bool!(shadow, "shadow");
    set_bool!(small_caps, "small_caps");
    set_bool!(all_caps, "all_caps");
    set_bool!(kerning, "kerning");

    if let Some(s) = get_str_from_map(&props_map, "font_name") {
        props.font_name = Some(s);
        any = true;
    }
    if let Some(s) = get_str_from_map(&props_map, "font_name_complex") {
        props.font_name_complex = Some(s);
        any = true;
    }
    if let Some(s) = get_str_from_map(&props_map, "font_name_east_asian") {
        props.font_name_east_asian = Some(s);
        any = true;
    }
    if let Some(v) = get_f64_from_map(&props_map, "font_size") {
        props.font_size = Some(Points::new(v));
        any = true;
    }
    if let Some(v) = get_f64_from_map(&props_map, "font_size_complex") {
        props.font_size_complex = Some(Points::new(v));
        any = true;
    }
    if let Some(v) = get_f64_from_map(&props_map, "scale") {
        props.scale = Some(v as f32);
        any = true;
    }
    if let Some(v) = get_f64_from_map(&props_map, "letter_spacing") {
        props.letter_spacing = Some(Points::new(v));
        any = true;
    }
    if let Some(v) = get_f64_from_map(&props_map, "word_spacing") {
        props.word_spacing = Some(Points::new(v));
        any = true;
    }
    if let Some(s) = get_str_from_map(&props_map, "language") {
        props.language = Some(LanguageTag::new(s));
        any = true;
    }
    if let Some(s) = get_str_from_map(&props_map, "language_complex") {
        props.language_complex = Some(LanguageTag::new(s));
        any = true;
    }
    if let Some(s) = get_str_from_map(&props_map, "language_east_asian") {
        props.language_east_asian = Some(LanguageTag::new(s));
        any = true;
    }
    if let Some(s) = get_str_from_map(&props_map, "underline")
        && let Some(u) = decode_underline(&s)
    {
        props.underline = Some(u);
        any = true;
    }
    if let Some(s) = get_str_from_map(&props_map, "strikethrough")
        && let Some(st) = decode_strikethrough(&s)
    {
        props.strikethrough = Some(st);
        any = true;
    }
    if let Some(s) = get_str_from_map(&props_map, "vertical_align")
        && let Some(va) = decode_vertical_align(&s)
    {
        props.vertical_align = Some(va);
        any = true;
    }
    if let Some(s) = get_str_from_map(&props_map, "hyperlink") {
        props.hyperlink = Some(s);
        any = true;
    }
    if let Some(s) = get_str_from_map(&props_map, "color")
        && let Ok(c) = DocumentColor::from_hex(&s)
    {
        props.color = Some(c);
        any = true;
    }
    if let Some(s) = get_str_from_map(&props_map, "background_color")
        && let Ok(c) = DocumentColor::from_hex(&s)
    {
        props.background_color = Some(c);
        any = true;
    }
    if let Some(s) = get_str_from_map(&props_map, "highlight_color")
        && let Some(h) = decode_highlight_color(&s)
    {
        props.highlight_color = Some(h);
        any = true;
    }

    if any { Some(props) } else { None }
}
