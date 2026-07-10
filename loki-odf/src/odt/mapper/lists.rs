// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! List-style mapper: converts [`OdfListStyle`]s into
//! format-neutral [`ListStyle`]s and inserts them into a [`StyleCatalog`].

use std::collections::HashMap;

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::list_style::{
    BulletChar, LabelAlignment, ListId, ListLevel, ListLevelKind, ListStyle, NumberingScheme,
};
use loki_primitives::units::Points;

use crate::odt::model::list_styles::{OdfListLevelKind, OdfListStyle};
use crate::version::OdfVersion;
use crate::xml_util::parse_length;

/// Convert all list styles in `sheet` and insert them into `catalog`.
///
/// Indentation is mapped using either the ODF 1.2+ label-alignment model
/// (when [`OdfVersion::supports_label_alignment`] returns `true` and
/// `label_followed_by` is `Some`) or the legacy ODF 1.1 model. `images`
/// (package path → (media-type, bytes)) resolves picture-bullet images to
/// `data:` URIs (feature 5.4).
pub(crate) fn map_list_styles(
    list_styles: &[OdfListStyle],
    catalog: &mut StyleCatalog,
    version: OdfVersion,
    images: &HashMap<String, (String, Vec<u8>)>,
) {
    for odf_ls in list_styles {
        let id = ListId::new(&odf_ls.name);
        let mut levels: Vec<ListLevel> = Vec::new();

        for odf_level in &odf_ls.levels {
            let level_num = odf_level.level + 1; // 0-indexed → 1-indexed ODF

            let kind = map_level_kind(&odf_level.kind, level_num, images);

            let (indent_start, hanging_indent) = map_indentation(odf_level, version);

            // Label char props from ODF text props on the level element
            let char_props = odf_level
                .text_props
                .as_ref()
                .map(crate::odt::mapper::props::map_text_props)
                .unwrap_or_default();

            levels.push(ListLevel {
                level: odf_level.level,
                kind,
                indent_start,
                hanging_indent,
                label_alignment: LabelAlignment::Left,
                tab_stop_after_label: odf_level
                    .list_tab_stop_position
                    .as_deref()
                    .and_then(parse_length),
                char_props,
            });
        }

        catalog.list_styles.insert(
            id.clone(),
            ListStyle {
                id,
                display_name: None,
                levels,
                extensions: ExtensionBag::default(),
            },
        );
    }
}

/// Map an [`OdfListLevelKind`] to the format-neutral [`ListLevelKind`].
fn map_level_kind(
    kind: &OdfListLevelKind,
    level_num: u8,
    images: &HashMap<String, (String, Vec<u8>)>,
) -> ListLevelKind {
    match kind {
        OdfListLevelKind::Bullet { char, style_name } => {
            let bullet_char = map_bullet_char(char);
            let font = style_name.clone();
            ListLevelKind::Bullet {
                char: bullet_char,
                font,
            }
        }
        OdfListLevelKind::Image { href, style_name } => {
            // Resolve the bullet image to a data URI; fall back to a text bullet
            // when the image is missing (feature 5.4).
            let char = match image_data_uri(href, images) {
                Some(src) => BulletChar::Image { src },
                None => BulletChar::Char('•'),
            };
            ListLevelKind::Bullet {
                char,
                font: style_name.clone(),
            }
        }
        OdfListLevelKind::Number {
            num_format,
            num_prefix,
            num_suffix,
            start_value,
            display_levels,
            ..
        } => {
            let raw_fmt = num_format.as_deref().unwrap_or("");
            if raw_fmt.is_empty() {
                return ListLevelKind::None;
            }
            let scheme = map_numbering_scheme(raw_fmt);
            let start = start_value.unwrap_or(1);
            let dl = *display_levels;
            let format =
                build_format_string(level_num, dl, num_prefix.as_ref(), num_suffix.as_ref());
            ListLevelKind::Numbered {
                scheme,
                start_value: start,
                format,
                display_levels: dl,
            }
        }
        OdfListLevelKind::None => ListLevelKind::None,
    }
}

/// Map an ODF bullet character string to [`BulletChar`].
///
/// Uses the first Unicode scalar of the string; falls back to `'•'` for
/// an empty string.
fn map_bullet_char(s: &str) -> BulletChar {
    BulletChar::Char(s.chars().next().unwrap_or('•'))
}

/// Resolve a `Pictures/…` href to a `data:` URI from the package image map, or
/// `None` when the part is absent.
fn image_data_uri(href: &str, images: &HashMap<String, (String, Vec<u8>)>) -> Option<String> {
    use base64::Engine as _;
    let (media_type, bytes) = images.get(href)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    Some(format!("data:{media_type};base64,{b64}"))
}

/// Map an ODF `style:num-format` value to [`NumberingScheme`].
///
/// Shared by list-level numbering and page-number formatting (`style:num-format`
/// on `style:page-layout-properties`) — both use the same ODF token set
/// (ODF 1.3 §20.396): `"1"` decimal, `"i"`/`"I"` lower/upper Roman,
/// `"a"`/`"A"` lower/upper letter.
pub(crate) fn map_numbering_scheme(s: &str) -> NumberingScheme {
    match s {
        "a" => NumberingScheme::LowerAlpha,
        "A" => NumberingScheme::UpperAlpha,
        "i" => NumberingScheme::LowerRoman,
        "I" => NumberingScheme::UpperRoman,
        _ => NumberingScheme::Decimal, // "1" or fallback
    }
}

/// Build the format string for a numbered level.
///
/// For `display_levels = 1`: `{prefix}%{level_num}{suffix}`
/// For `display_levels > 1`: `%{start}.%{start+1}...%{level_num}{suffix}`
/// where `start = level_num - display_levels + 1`.
fn build_format_string(
    level_num: u8,
    display_levels: u8,
    prefix: Option<&String>,
    suffix: Option<&String>,
) -> String {
    let suffix_str = suffix.map_or("", String::as_str);
    if display_levels <= 1 {
        let prefix_str = prefix.map_or("", String::as_str);
        format!("{prefix_str}%{level_num}{suffix_str}")
    } else {
        // Both level_num and display_levels are u8, so the result fits in u8.
        #[allow(clippy::cast_possible_truncation)]
        let start = u16::from(level_num).saturating_sub(u16::from(display_levels) - 1) as u8;
        let mut s = String::new();
        for l in start..=level_num {
            s.push('%');
            s.push_str(&l.to_string());
            if l < level_num {
                s.push('.');
            }
        }
        s.push_str(suffix_str);
        s
    }
}

/// Map positioning attributes to `(indent_start, hanging_indent)`.
///
/// Uses the ODF 1.2+ label-alignment model when the version supports it
/// and `label_followed_by` is `Some`; otherwise falls back to the legacy
/// ODF 1.1 `text:space-before` / `text:min-label-width` model.
fn map_indentation(
    level: &crate::odt::model::list_styles::OdfListLevel,
    version: OdfVersion,
) -> (Points, Points) {
    let zero = Points::new(0.0);

    if version.supports_label_alignment() && level.label_followed_by.is_some() {
        // ODF 1.2+ label-alignment model
        let indent_start = level
            .margin_left
            .as_deref()
            .and_then(parse_length)
            .unwrap_or(zero);
        // text_indent is negative (hanging); store positive
        let hanging = level
            .text_indent
            .as_deref()
            .and_then(parse_length)
            .map_or(zero, |p| {
                if p.value() < 0.0 {
                    Points::new(-p.value())
                } else {
                    p
                }
            });
        (indent_start, hanging)
    } else {
        // ODF 1.1 legacy model: space_before + min_label_width = total indent
        let space_before = level
            .legacy_space_before
            .as_deref()
            .and_then(parse_length)
            .unwrap_or(zero);
        let label_width = level
            .legacy_min_label_width
            .as_deref()
            .and_then(parse_length)
            .unwrap_or(zero);
        let indent_start = Points::new(space_before.value() + label_width.value());
        let hanging = label_width;
        (indent_start, hanging)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "lists_tests.rs"]
mod tests;
