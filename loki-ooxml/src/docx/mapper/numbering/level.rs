// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Mapping from [`DocxLevel`] to [`ListLevel`] and [`ListLevelKind`].

use loki_doc_model::style::list_style::{BulletChar, ListLevel, ListLevelKind, NumberingScheme};
use loki_primitives::units::Points;

use crate::docx::mapper::props::map_rpr;
use crate::docx::model::numbering::DocxLevel;

use super::helpers::{count_display_levels, map_lvl_jc, normalize_bullet_char};

/// Maps a [`DocxLevel`] to a [`ListLevel`].
pub(super) fn map_level(lvl: &DocxLevel, start_override: Option<u32>) -> ListLevel {
    let indent_start = lvl
        .ppr
        .as_ref()
        .and_then(|p| p.ind.as_ref())
        .and_then(|i| i.left)
        .map_or(Points::new(0.0), |v| Points::new(f64::from(v) / 20.0));

    let hanging_indent = lvl
        .ppr
        .as_ref()
        .and_then(|p| p.ind.as_ref())
        .and_then(|i| i.hanging)
        .map_or(Points::new(0.0), |v| Points::new(f64::from(v) / 20.0));

    let char_props = lvl.rpr.as_ref().map(map_rpr).unwrap_or_default();

    let label_alignment = map_lvl_jc(lvl.lvl_jc.as_deref());

    let start_value = start_override.or(lvl.start).unwrap_or(1);

    let kind = map_level_kind(lvl, start_value, &char_props);

    ListLevel {
        level: lvl.ilvl,
        kind,
        indent_start,
        hanging_indent,
        label_alignment,
        tab_stop_after_label: None,
        char_props,
    }
}

/// Maps `w:numFmt` + `w:lvlText` + start to a [`ListLevelKind`].
fn map_level_kind(
    lvl: &DocxLevel,
    start_value: u32,
    char_props: &loki_doc_model::style::props::char_props::CharProps,
) -> ListLevelKind {
    let num_fmt = lvl.num_fmt.as_deref().unwrap_or("decimal");
    let lvl_text = lvl.lvl_text.as_deref();
    let font = char_props.font_name.clone();

    match num_fmt {
        "bullet" => {
            let bullet_char = match lvl_text {
                Some("•") | None => BulletChar::Char('•'),
                Some("–") => BulletChar::Char('–'),
                Some("○") => BulletChar::Char('○'),
                Some("▪") => BulletChar::Char('▪'),
                Some(s) => {
                    let raw = s.chars().next().unwrap_or('•');
                    BulletChar::Char(normalize_bullet_char(raw))
                }
            };
            ListLevelKind::Bullet {
                char: bullet_char,
                font,
            }
        }
        "none" => ListLevelKind::None,
        _ => {
            let scheme = match num_fmt {
                "lowerLetter" => NumberingScheme::LowerAlpha,
                "upperLetter" => NumberingScheme::UpperAlpha,
                "lowerRoman" => NumberingScheme::LowerRoman,
                "upperRoman" => NumberingScheme::UpperRoman,
                "ordinal" => NumberingScheme::Ordinal,
                _ => NumberingScheme::Decimal,
            };
            let format = lvl_text.unwrap_or("%1.").to_owned();
            let display_levels = count_display_levels(&format).max(1);
            ListLevelKind::Numbered {
                scheme,
                start_value,
                format,
                display_levels,
            }
        }
    }
}
