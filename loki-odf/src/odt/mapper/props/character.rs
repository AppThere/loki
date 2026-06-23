// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Character property mapping (`OdfTextProps` → `CharProps`).

use loki_doc_model::meta::LanguageTag;
use loki_doc_model::style::props::char_props::{
    CharProps, StrikethroughStyle, UnderlineStyle, VerticalAlign,
};
use loki_primitives::color::DocumentColor;

use crate::odt::model::styles::OdfTextProps;
use crate::xml_util::parse_length;

// ── Character properties ──────────────────────────────────────────────────────

/// Convert [`OdfTextProps`] to the format-neutral [`CharProps`].
///
/// ODF 1.3 §20.2 (`style:text-properties`).
pub(crate) fn map_text_props(props: &OdfTextProps) -> CharProps {
    // ── Font ───────────────────────────────────────────────────────────────
    // Prefer style:font-name (the font face alias, typically matching the actual
    // family name); fall back to fo:font-family when only that is present.
    let mut out = CharProps {
        font_name: props
            .font_name
            .clone()
            .or_else(|| props.font_family.clone()),
        font_name_complex: props.font_name_complex.clone(),
        font_name_east_asian: props.font_name_asian.clone(),
        outline: props.text_outline,
        ..Default::default()
    };

    if let Some(pts) = props.font_size.as_deref().and_then(parse_length) {
        out.font_size = Some(pts);
    }
    if let Some(pts) = props.font_size_complex.as_deref().and_then(parse_length) {
        out.font_size_complex = Some(pts);
    }

    // ── Style flags ────────────────────────────────────────────────────────
    out.bold = match props.font_weight.as_deref() {
        Some("bold") => Some(true),
        Some("normal") => Some(false),
        _ => None,
    };
    out.italic = match props.font_style.as_deref() {
        Some("italic" | "oblique") => Some(true),
        Some("normal") => Some(false),
        _ => None,
    };
    out.underline = props
        .text_underline_style
        .as_deref()
        .and_then(map_underline_style);
    out.strikethrough = props
        .text_line_through_style
        .as_deref()
        .and_then(map_strikethrough_style);

    // ── Case / variant ─────────────────────────────────────────────────────
    if props.font_variant.as_deref() == Some("small-caps") {
        out.small_caps = Some(true);
    }
    if props.text_transform.as_deref() == Some("uppercase") {
        out.all_caps = Some(true);
    }

    // ── Vertical alignment (super/subscript) ───────────────────────────────
    if let Some(pos) = props.text_position.as_deref() {
        out.vertical_align = map_text_position(pos);
    }

    // ── Color ──────────────────────────────────────────────────────────────
    if let Some(hex) = props.color.as_deref()
        && let Ok(dc) = DocumentColor::from_hex(hex)
    {
        out.color = Some(dc);
    }
    if let Some(hex) = props.background_color.as_deref()
        && hex != "transparent"
        && let Ok(dc) = DocumentColor::from_hex(hex)
    {
        out.background_color = Some(dc);
    }

    // ── Shadow ─────────────────────────────────────────────────────────────
    // ODF fo:text-shadow is a CSS shadow string; any non-empty, non-"none"
    // value means shadow is enabled.
    if let Some(shadow) = props.text_shadow.as_deref() {
        out.shadow = Some(!shadow.is_empty() && shadow != "none");
    }

    // ── Spacing ────────────────────────────────────────────────────────────
    if let Some(pts) = props.letter_spacing.as_deref().and_then(parse_length) {
        out.letter_spacing = Some(pts);
    }
    if let Some(pts) = props.word_spacing.as_deref().and_then(parse_length) {
        out.word_spacing = Some(pts);
    }
    if let Some(v) = props.letter_kerning {
        out.kerning = Some(v);
    }
    // style:text-scale is a percentage string like "150%" → 150.0 (same unit as OOXML w:w)
    if let Some(pct) = props.text_scale.as_deref()
        && let Some(v) = pct.strip_suffix('%').and_then(|s| s.parse::<f32>().ok())
    {
        out.scale = Some(v);
    }

    // ── Language ───────────────────────────────────────────────────────────
    if let Some(lang) = props.language.as_deref() {
        let tag = if let Some(country) = props.country.as_deref() {
            LanguageTag::new(format!("{lang}-{country}"))
        } else {
            LanguageTag::new(lang)
        };
        out.language = Some(tag);
    }
    if let Some(lang) = props.language_complex.as_deref() {
        let tag = if let Some(country) = props.country_complex.as_deref() {
            LanguageTag::new(format!("{lang}-{country}"))
        } else {
            LanguageTag::new(lang)
        };
        out.language_complex = Some(tag);
    }
    if let Some(lang) = props.language_asian.as_deref() {
        let tag = if let Some(country) = props.country_asian.as_deref() {
            LanguageTag::new(format!("{lang}-{country}"))
        } else {
            LanguageTag::new(lang)
        };
        out.language_east_asian = Some(tag);
    }

    out
}

/// Map ODF `style:text-underline-style` to [`UnderlineStyle`].
///
/// `"none"` → `None` (explicit removal). All other recognised values map to
/// a concrete style; unrecognised values map to [`UnderlineStyle::Single`].
fn map_underline_style(s: &str) -> Option<UnderlineStyle> {
    match s {
        "none" => None,
        "double" => Some(UnderlineStyle::Double),
        "dotted" => Some(UnderlineStyle::Dotted),
        "dash" | "long-dash" | "dot-dash" | "dot-dot-dash" => Some(UnderlineStyle::Dash),
        "wave" => Some(UnderlineStyle::Wave),
        "bold" => Some(UnderlineStyle::Thick),
        _ => Some(UnderlineStyle::Single),
    }
}

/// Map ODF `style:text-line-through-style` to [`StrikethroughStyle`].
///
/// `"none"` → `None`. `"double"` → `Double`. All other values → `Single`.
fn map_strikethrough_style(s: &str) -> Option<StrikethroughStyle> {
    match s {
        "none" => None,
        "double" => Some(StrikethroughStyle::Double),
        _ => Some(StrikethroughStyle::Single),
    }
}

/// Map ODF `style:text-position` to [`VerticalAlign`].
///
/// Recognised forms: `"super"`, `"sub"`, percentage strings (positive =
/// superscript, negative = subscript), or a percentage followed by a font
/// size (the second token is ignored). ODF 1.3 §19.879.
fn map_text_position(s: &str) -> Option<VerticalAlign> {
    let first = s.split_whitespace().next().unwrap_or(s);
    match first {
        "super" => Some(VerticalAlign::Superscript),
        "sub" => Some(VerticalAlign::Subscript),
        "0%" | "0" => Some(VerticalAlign::Baseline),
        other => {
            // Percentage string: positive → super, negative → sub
            if let Some(pct_str) = other.strip_suffix('%')
                && let Ok(pct) = pct_str.parse::<f32>()
            {
                return if pct > 0.0 {
                    Some(VerticalAlign::Superscript)
                } else if pct < 0.0 {
                    Some(VerticalAlign::Subscript)
                } else {
                    Some(VerticalAlign::Baseline)
                };
            }
            None
        }
    }
}
