// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Inline content serialization and deserialization for the Loro bridge.

use loro::{LoroText, LoroValue};
use loki_primitives::units::Points;
use crate::content::inline::{Inline, StyledRun};
use crate::content::attr::NodeAttr;
use crate::style::props::char_props::{CharProps, UnderlineStyle, StrikethroughStyle, VerticalAlign};
use crate::loro_schema::*;
use super::BridgeError;

// ── Serialization ─────────────────────────────────────────────────────────────

pub(super) fn map_inlines(inlines: &[Inline], text: &LoroText) -> Result<(), BridgeError> {
    for inline in inlines {
        let start = text.len_unicode();
        match inline {
            Inline::Str(s) => {
                text.insert(start, s)?;
            }
            Inline::Space => {
                text.insert(start, " ")?;
            }
            Inline::SoftBreak | Inline::LineBreak => {
                text.insert(start, "\n")?;
            }
            Inline::StyledRun(run) => {
                let text_str = extract_plain_text(&run.content);
                text.insert(start, &text_str)?;
                let end = text.len_unicode();
                if start < end && let Some(props) = &run.direct_props {
                    apply_char_props_marks(props, start, end, text)?;
                }
            }
            Inline::Emph(inner) => {
                let text_str = extract_plain_text(inner);
                text.insert(start, &text_str)?;
                let end = text.len_unicode();
                if start < end {
                    text.mark(start..end, MARK_ITALIC, true)?;
                }
            }
            Inline::Strong(inner) => {
                let text_str = extract_plain_text(inner);
                text.insert(start, &text_str)?;
                let end = text.len_unicode();
                if start < end {
                    text.mark(start..end, MARK_BOLD, true)?;
                }
            }
            Inline::Underline(inner) => {
                let text_str = extract_plain_text(inner);
                text.insert(start, &text_str)?;
                let end = text.len_unicode();
                if start < end {
                    text.mark(start..end, MARK_UNDERLINE, "Single")?;
                }
            }
            _ => {
                let text_str = extract_plain_text(std::slice::from_ref(inline));
                if !text_str.is_empty() {
                    text.insert(start, &text_str)?;
                }
            }
        }
    }
    Ok(())
}

pub(super) fn extract_plain_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            Inline::Str(s) => out.push_str(s),
            Inline::Space => out.push(' '),
            Inline::SoftBreak | Inline::LineBreak => out.push('\n'),
            Inline::StyledRun(run) => out.push_str(&extract_plain_text(&run.content)),
            Inline::Emph(inner) => out.push_str(&extract_plain_text(inner)),
            Inline::Strong(inner) => out.push_str(&extract_plain_text(inner)),
            Inline::Underline(inner) => out.push_str(&extract_plain_text(inner)),
            _ => {}
        }
    }
    out
}

pub(super) fn apply_char_props_marks(
    props: &CharProps,
    start: usize,
    end: usize,
    text: &LoroText,
) -> Result<(), BridgeError> {
    if let Some(v) = props.bold { text.mark(start..end, MARK_BOLD, v)?; }
    if let Some(v) = props.italic { text.mark(start..end, MARK_ITALIC, v)?; }
    if let Some(v) = props.outline { text.mark(start..end, MARK_OUTLINE, v)?; }
    if let Some(v) = props.shadow { text.mark(start..end, MARK_SHADOW, v)?; }
    if let Some(v) = props.small_caps { text.mark(start..end, MARK_SMALL_CAPS, v)?; }
    if let Some(v) = props.all_caps { text.mark(start..end, MARK_ALL_CAPS, v)?; }
    if let Some(v) = props.kerning { text.mark(start..end, MARK_KERNING, v)?; }
    if let Some(v) = &props.font_name { text.mark(start..end, MARK_FONT_FAMILY, v.clone())?; }
    if let Some(v) = &props.font_size { text.mark(start..end, MARK_FONT_SIZE_PT, v.value())?; }
    if let Some(v) = props.scale { text.mark(start..end, MARK_SCALE, f64::from(v))?; }
    if let Some(v) = &props.letter_spacing { text.mark(start..end, MARK_LETTER_SPACING, v.value())?; }
    if let Some(v) = &props.word_spacing { text.mark(start..end, MARK_WORD_SPACING, v.value())?; }
    if let Some(v) = &props.underline { text.mark(start..end, MARK_UNDERLINE, format!("{:?}", v))?; }
    if let Some(v) = &props.strikethrough { text.mark(start..end, MARK_STRIKETHROUGH, format!("{:?}", v))?; }
    if let Some(v) = &props.vertical_align { text.mark(start..end, MARK_VERTICAL_ALIGN, format!("{:?}", v))?; }
    if let Some(v) = &props.color { text.mark(start..end, MARK_COLOR, format!("{:?}", v))?; }
    if let Some(v) = &props.highlight_color { text.mark(start..end, MARK_HIGHLIGHT_COLOR, format!("{:?}", v))?; }
    if let Some(v) = &props.language { text.mark(start..end, MARK_LANGUAGE, format!("{:?}", v))?; }
    if let Some(v) = &props.hyperlink { text.mark(start..end, MARK_LINK_URL, v.clone())?; }
    Ok(())
}

// ── Deserialization ───────────────────────────────────────────────────────────

pub(super) fn reconstruct_inlines(
    map: &loro::LoroMap,
) -> Result<Vec<Inline>, BridgeError> {
    let mut inlines = Vec::new();
    let Some(content_val) = map.get(KEY_CONTENT) else { return Ok(inlines); };
    let Some(text_container) = content_val.into_container().ok().and_then(|c| c.into_text().ok()) else {
        return Ok(inlines);
    };

    for span in text_container.to_delta() {
        if let loro::TextDelta::Insert { insert, attributes } = span {
            match attributes {
                None => inlines.push(Inline::Str(insert.to_string())),
                Some(attrs) => {
                    let props = read_char_props_from_marks(&attrs);
                    if props.is_some() {
                        let run = StyledRun {
                            style_id: None,
                            direct_props: props.map(Box::new),
                            content: vec![Inline::Str(insert.to_string())],
                            attr: NodeAttr::default(),
                        };
                        inlines.push(Inline::StyledRun(run));
                    } else {
                        inlines.push(Inline::Str(insert.to_string()));
                    }
                }
            }
        }
    }
    Ok(inlines)
}

fn read_char_props_from_marks(
    attrs: &rustc_hash::FxHashMap<String, LoroValue>,
) -> Option<CharProps> {
    let mut props = CharProps::default();
    let mut any = false;

    macro_rules! read_bool {
        ($field:ident, $key:expr) => {
            if let Some(LoroValue::Bool(v)) = attrs.get($key) {
                props.$field = Some(*v);
                any = true;
            }
        };
    }
    macro_rules! read_f64 {
        ($field:ident, $key:expr, $map:expr) => {
            if let Some(LoroValue::Double(v)) = attrs.get($key) {
                props.$field = Some($map(*v));
                any = true;
            }
        };
    }
    macro_rules! read_str {
        ($field:ident, $key:expr, $decode:expr) => {
            if let Some(LoroValue::String(s)) = attrs.get($key) {
                if let Some(v) = $decode(s.as_str()) {
                    props.$field = Some(v);
                    any = true;
                }
            }
        };
    }

    read_bool!(bold, MARK_BOLD);
    read_bool!(italic, MARK_ITALIC);
    read_bool!(outline, MARK_OUTLINE);
    read_bool!(shadow, MARK_SHADOW);
    read_bool!(small_caps, MARK_SMALL_CAPS);
    read_bool!(all_caps, MARK_ALL_CAPS);
    read_bool!(kerning, MARK_KERNING);

    read_f64!(font_size, MARK_FONT_SIZE_PT, Points::new);
    read_f64!(scale, MARK_SCALE, |v: f64| v as f32);
    read_f64!(letter_spacing, MARK_LETTER_SPACING, Points::new);
    read_f64!(word_spacing, MARK_WORD_SPACING, Points::new);

    if let Some(LoroValue::String(s)) = attrs.get(MARK_FONT_FAMILY) {
        props.font_name = Some(s.to_string());
        any = true;
    }
    if let Some(LoroValue::String(s)) = attrs.get(MARK_LINK_URL) {
        props.hyperlink = Some(s.to_string());
        any = true;
    }

    read_str!(underline, MARK_UNDERLINE, decode_underline);
    read_str!(strikethrough, MARK_STRIKETHROUGH, decode_strikethrough);
    read_str!(vertical_align, MARK_VERTICAL_ALIGN, decode_vertical_align);
    // color, highlight_color, language — complex Debug-format strings; deferred
    // TODO(loro-bridge): decode color/highlight_color/language marks

    if any { Some(props) } else { None }
}

// ── Enum decode helpers ───────────────────────────────────────────────────────

pub(super) fn decode_underline(s: &str) -> Option<UnderlineStyle> {
    match s {
        "Single" => Some(UnderlineStyle::Single),
        "Double" => Some(UnderlineStyle::Double),
        "Dotted" => Some(UnderlineStyle::Dotted),
        "Dash" => Some(UnderlineStyle::Dash),
        "Wave" => Some(UnderlineStyle::Wave),
        "Thick" => Some(UnderlineStyle::Thick),
        _ => None,
    }
}

pub(super) fn decode_strikethrough(s: &str) -> Option<StrikethroughStyle> {
    match s {
        "Single" => Some(StrikethroughStyle::Single),
        "Double" => Some(StrikethroughStyle::Double),
        _ => None,
    }
}

pub(super) fn decode_vertical_align(s: &str) -> Option<VerticalAlign> {
    match s {
        "Superscript" => Some(VerticalAlign::Superscript),
        "Subscript" => Some(VerticalAlign::Subscript),
        "Baseline" => Some(VerticalAlign::Baseline),
        _ => None,
    }
}
