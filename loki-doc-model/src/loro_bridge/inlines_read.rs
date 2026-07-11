// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Inline content deserialization for the Loro bridge (read direction).
//!
//! Serialization lives in `inlines.rs`; the two files are split to stay
//! under the 300-line ceiling.

use super::BridgeError;
use super::color_codec::decode_document_color;
use super::decode::{
    decode_highlight_color, decode_strikethrough, decode_underline, decode_vertical_align,
};
use crate::content::attr::NodeAttr;
use crate::content::inline::{Inline, QuoteType, StyledRun};
use crate::loro_schema::*;
use crate::style::catalog::StyleId;
use crate::style::props::char_props::CharProps;
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

    // Live footnote/endnote bodies live in a side-container on the block map.
    let notes_list = map
        .get(KEY_NOTES)
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_movable_list().ok());

    for span in text_container.to_delta() {
        if let loro::TextDelta::Insert { insert, attributes } = span {
            match attributes {
                None => inlines.push(Inline::Str(insert.to_string())),
                Some(attrs) => {
                    // An inline object (anchored by a placeholder char) carries
                    // its data in a mark — reconstruct it and discard the
                    // placeholder character itself. Image data rides in the
                    // mark; a note's body is fetched from the notes container.
                    if let Some(image) = decode_image(&attrs) {
                        inlines.push(image);
                        continue;
                    }
                    if let Some(note) = decode_note(&attrs, notes_list.as_ref()) {
                        inlines.push(note);
                        continue;
                    }
                    if let Some(anchor) = decode_snapshot_anchor(&attrs, MARK_COMMENT, |i| {
                        matches!(i, Inline::Comment(_))
                    }) {
                        inlines.push(anchor);
                        continue;
                    }
                    if let Some(anchor) = decode_snapshot_anchor(&attrs, MARK_BOOKMARK, |i| {
                        matches!(i, Inline::Bookmark(_, _))
                    }) {
                        inlines.push(anchor);
                        continue;
                    }
                    let props = read_char_props_from_marks(&attrs);
                    let style_id = read_style_id_from_marks(&attrs);
                    let mut inline = if props.is_some() || style_id.is_some() {
                        Inline::StyledRun(StyledRun {
                            style_id,
                            direct_props: props.map(Box::new),
                            content: vec![Inline::Str(insert.to_string())],
                            attr: NodeAttr::default(),
                        })
                    } else {
                        Inline::Str(insert.to_string())
                    };
                    // Re-wrap span/quote range marks (innermost first, so a
                    // quoted span reads back as Quoted(Span(..)) — the write
                    // path flattens both onto the same range).
                    if let Some(attr) = decode_span_attr(&attrs) {
                        inline = Inline::Span(attr, vec![inline]);
                    }
                    if let Some(qt) = decode_quote_type(&attrs) {
                        inline = Inline::Quoted(qt, vec![inline]);
                    }
                    inlines.push(inline);
                }
            }
        }
    }
    Ok(inlines)
}

/// Reconstructs an inline from a snapshot-anchor mark (`mark_key` holding a
/// `serde`-JSON `Inline`), if present and of the expected variant (`want`).
/// Returns `None` for ordinary formatted text.
#[cfg(feature = "serde")]
fn decode_snapshot_anchor(
    attrs: &rustc_hash::FxHashMap<String, LoroValue>,
    mark_key: &str,
    want: fn(&Inline) -> bool,
) -> Option<Inline> {
    let Some(LoroValue::String(json)) = attrs.get(mark_key) else {
        return None;
    };
    match serde_json::from_str::<Inline>(json) {
        Ok(inline) if want(&inline) => Some(inline),
        Ok(_) => {
            tracing::warn!("loro bridge: {mark_key} snapshot was the wrong inline variant");
            None
        }
        Err(err) => {
            tracing::warn!("loro bridge: failed to decode {mark_key} anchor: {err}");
            None
        }
    }
}

#[cfg(not(feature = "serde"))]
fn decode_snapshot_anchor(
    _attrs: &rustc_hash::FxHashMap<String, LoroValue>,
    _mark_key: &str,
    _want: fn(&Inline) -> bool,
) -> Option<Inline> {
    None
}

/// Reconstructs an [`Inline::Image`] from a [`MARK_IMAGE`] anchor's `serde`-JSON
/// snapshot, if present. Returns `None` for ordinary formatted text.
#[cfg(feature = "serde")]
fn decode_image(attrs: &rustc_hash::FxHashMap<String, LoroValue>) -> Option<Inline> {
    decode_snapshot_anchor(attrs, MARK_IMAGE, |i| matches!(i, Inline::Image(..)))
}

/// Reconstructs an [`Inline::Note`] from a [`MARK_NOTE`] anchor: its `(kind,
/// idx)` mark plus the body fetched from `notes` at `idx` (a live container).
#[cfg(feature = "serde")]
fn decode_note(
    attrs: &rustc_hash::FxHashMap<String, LoroValue>,
    notes: Option<&loro::LoroMovableList>,
) -> Option<Inline> {
    use crate::content::inline::NoteKind;
    let Some(LoroValue::String(meta)) = attrs.get(MARK_NOTE) else {
        return None;
    };
    let (kind, idx): (NoteKind, usize) = serde_json::from_str(meta).ok()?;
    let body = notes
        .and_then(|l| l.get(idx))
        .and_then(|v| v.into_container().ok())
        .and_then(|c| c.into_movable_list().ok())
        .map(|l| super::read::reconstruct_blocks_from_list(&l))
        .unwrap_or_default();
    Some(Inline::Note(kind, body))
}

#[cfg(not(feature = "serde"))]
fn decode_image(_attrs: &rustc_hash::FxHashMap<String, LoroValue>) -> Option<Inline> {
    None
}

#[cfg(not(feature = "serde"))]
fn decode_note(
    _attrs: &rustc_hash::FxHashMap<String, LoroValue>,
    _notes: Option<&loro::LoroMovableList>,
) -> Option<Inline> {
    None
}

/// Reads the quote style carried by [`MARK_QUOTE_TYPE`].
fn decode_quote_type(attrs: &rustc_hash::FxHashMap<String, LoroValue>) -> Option<QuoteType> {
    match attrs.get(MARK_QUOTE_TYPE) {
        Some(LoroValue::String(s)) if s.as_str() == "Single" => Some(QuoteType::SingleQuote),
        Some(LoroValue::String(s)) if s.as_str() == "Double" => Some(QuoteType::DoubleQuote),
        _ => None,
    }
}

/// Reads the span attributes carried by [`MARK_SPAN_ATTR`].
#[cfg(feature = "serde")]
fn decode_span_attr(attrs: &rustc_hash::FxHashMap<String, LoroValue>) -> Option<NodeAttr> {
    let Some(LoroValue::String(json)) = attrs.get(MARK_SPAN_ATTR) else {
        return None;
    };
    serde_json::from_str(json).ok()
}

#[cfg(not(feature = "serde"))]
fn decode_span_attr(_attrs: &rustc_hash::FxHashMap<String, LoroValue>) -> Option<NodeAttr> {
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
    read_str!(color, MARK_COLOR, decode_document_color);
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
    if let Some(LoroValue::String(s)) = attrs.get(MARK_REVISION)
        && let Some(rev) = crate::style::props::revision::decode(s.as_str())
    {
        props.revision = Some(rev);
        any = true;
    }

    if any { Some(props) } else { None }
}
