// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Mapping of ODF list-level kinds to format-neutral [`ListLevelKind`]s.

use loki_doc_model::style::list_style::{BulletChar, ListLevelKind, NumberingScheme};

use crate::odt::model::list_styles::OdfListLevelKind;

/// Map an [`OdfListLevelKind`] to the format-neutral [`ListLevelKind`].
pub(super) fn map_level_kind(kind: &OdfListLevelKind, level_num: u8) -> ListLevelKind {
    match kind {
        OdfListLevelKind::Bullet { char, style_name } => {
            let bullet_char = map_bullet_char(char);
            let font = style_name.clone();
            ListLevelKind::Bullet {
                char: bullet_char,
                font,
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

/// Map an ODF `style:num-format` value to [`NumberingScheme`].
fn map_numbering_scheme(s: &str) -> NumberingScheme {
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
pub(super) fn build_format_string(
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
