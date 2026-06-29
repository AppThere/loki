// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Inline content deserialization for the Loro bridge (read direction).
//!
//! Serialization lives in `inlines.rs`; the two files are split to stay
//! under the 300-line ceiling.

use super::BridgeError;
use super::decode::{
    decode_highlight_color, decode_strikethrough, decode_underline, decode_vertical_align,
};
use crate::content::attr::NodeAttr;
use crate::content::inline::{Inline, StyledRun};
use crate::loro_schema::*;
use crate::style::catalog::StyleId;
use crate::style::props::char_props::CharProps;
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;
use loro::LoroValue;

pub(super) fn reconstruct_inlines(map: &loro::LoroMap) -> Result<Vec<Inline>, BridgeError> {
    let mut inlines = Vec::new();
    let Some(content_val) = map.get(KEY_CONTENT) else {
        return Ok(inlines);
    };
    let Some(text_container) = content_val
        .into_container()
        .ok()
        .and_then(|c| c.into_text().ok())
    else {
        return Ok(inlines);
    };

    for span in text_container.to_delta() {
        if let loro::TextDelta::Insert { insert, attributes } = span {
            match attributes {
                None => inlines.push(Inline::Str(insert.to_string())),
                Some(attrs) => {
                    // An inline object (anchored by a placeholder char) carries
                    // its structured data in a mark: reconstruct the object and
                    // discard the placeholder character itself.
                    if let Some(object) = decode_inline_object(&attrs) {
                        inlines.push(object);
                        continue;
                    }
                    let props = read_char_props_from_marks(&attrs);
                    let style_id = read_style_id_from_marks(&attrs);
                    if props.is_some() || style_id.is_some() {
                        let run = StyledRun {
                            style_id,
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

/// Reconstructs an inline object from an anchor span's marks, if present.
///
/// Handles [`MARK_IMAGE`] (`Inline::Image`) and [`MARK_NOTE`] (`Inline::Note`),
/// each a `serde`-JSON snapshot. Returns `None` for ordinary formatted text so
/// the caller falls through to the char-props / style path.
#[cfg(feature = "serde")]
fn decode_inline_object(attrs: &rustc_hash::FxHashMap<String, LoroValue>) -> Option<Inline> {
    decode_object_mark(attrs, MARK_IMAGE, |i| matches!(i, Inline::Image(..)))
        .or_else(|| decode_object_mark(attrs, MARK_NOTE, |i| matches!(i, Inline::Note(..))))
}

/// Decodes the `serde`-JSON `Inline` snapshot stored under `key`, accepting it
/// only when `is_expected` confirms the variant matches the mark.
#[cfg(feature = "serde")]
fn decode_object_mark(
    attrs: &rustc_hash::FxHashMap<String, LoroValue>,
    key: &str,
    is_expected: impl Fn(&Inline) -> bool,
) -> Option<Inline> {
    let Some(LoroValue::String(json)) = attrs.get(key) else {
        return None;
    };
    match serde_json::from_str::<Inline>(json) {
        Ok(inline) if is_expected(&inline) => Some(inline),
        Ok(_) => {
            tracing::warn!("loro bridge: {key} snapshot had an unexpected inline variant");
            None
        }
        Err(err) => {
            tracing::warn!("loro bridge: failed to decode inline object ({key}): {err}");
            None
        }
    }
}

#[cfg(not(feature = "serde"))]
fn decode_inline_object(_attrs: &rustc_hash::FxHashMap<String, LoroValue>) -> Option<Inline> {
    None
}

/// Reads the named character style carried by [`MARK_CHAR_STYLE_ID`].
fn read_style_id_from_marks(attrs: &rustc_hash::FxHashMap<String, LoroValue>) -> Option<StyleId> {
    if let Some(LoroValue::String(s)) = attrs.get(MARK_CHAR_STYLE_ID) {
        Some(StyleId(s.to_string()))
    } else {
        None
    }
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
    read_str!(color, MARK_COLOR, |s: &str| DocumentColor::from_hex(s).ok());
    read_str!(
        highlight_color,
        MARK_HIGHLIGHT_COLOR,
        decode_highlight_color
    );
    if let Some(LoroValue::String(s)) = attrs.get(MARK_LANGUAGE) {
        props.language = Some(crate::meta::language::LanguageTag::new(s.to_string()));
        any = true;
    }
    if let Some(LoroValue::String(s)) = attrs.get(MARK_LANGUAGE_COMPLEX) {
        props.language_complex = Some(crate::meta::language::LanguageTag::new(s.to_string()));
        any = true;
    }
    if let Some(LoroValue::String(s)) = attrs.get(MARK_LANGUAGE_EAST_ASIAN) {
        props.language_east_asian = Some(crate::meta::language::LanguageTag::new(s.to_string()));
        any = true;
    }

    if any { Some(props) } else { None }
}
