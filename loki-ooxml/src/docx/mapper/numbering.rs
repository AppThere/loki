// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Numbering mapper: [`DocxNumbering`] → [`ListStyle`] entries in a [`StyleCatalog`].
//!
//! OOXML uses three-level indirection: paragraph `w:numId` → `w:num`
//! → `w:abstractNum`. Each `w:num` instance may override individual levels
//! via `w:lvlOverride`. ECMA-376 §17.9.

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::list_style::{
    BulletChar, LabelAlignment, ListId, ListLevel, ListLevelKind, ListStyle, NumberingScheme,
};
use loki_primitives::units::Points;

use crate::docx::model::numbering::{DocxLevel, DocxNumbering};
use crate::error::OoxmlWarning;

use super::props::map_rpr;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Maps a `w:lvlJc` value to [`LabelAlignment`].
fn map_lvl_jc(jc: Option<&str>) -> LabelAlignment {
    match jc {
        Some("right" | "end") => LabelAlignment::Right,
        Some("center") => LabelAlignment::Center,
        _ => LabelAlignment::Left,
    }
}

/// Normalize a bullet character, remapping Wingdings/Symbol PUA code points
/// (U+E000..U+F8FF) to the closest standard Unicode equivalent.
///
/// DOCX files produced by Microsoft Word commonly store bullet characters as
/// Private Use Area code points such as U+F0B7 (Wingdings bullet).  These only
/// render correctly when the Wingdings or Symbol font is explicitly applied.
/// Because the layout engine uses a plain `Inline::Str` for the marker (no
/// per-run font override), the character is shaped with the paragraph's default
/// font, which has no glyph for PUA code points → tofu.
///
/// Remapping to standard Unicode lets any font render the bullet correctly.
fn normalize_bullet_char(c: char) -> char {
    match c {
        '\u{F06C}' | '\u{F076}' | '\u{F0D8}' => '●', // Wingdings: filled circle → BLACK CIRCLE
        '\u{F06E}' => '○',                           // Wingdings: white circle → WHITE CIRCLE
        '\u{F0FC}' => '■',                           // Wingdings: filled square → BLACK SQUARE
        '\u{F0E8}' => '✓',                           // Wingdings 2: check mark → CHECK MARK
        '\u{F067}' => '–',                           // Wingdings: dash → EN DASH
        // Bullet variants (F0B7/F0A7) and all remaining PUA chars fall back to bullet.
        '\u{E000}'..='\u{F8FF}' => '•',
        _ => c,
    }
}

/// Counts `%N` tokens in a level-text format string (e.g. `"%1.%2."` → 2).
fn count_display_levels(lvl_text: &str) -> u8 {
    // A %N token is '%' followed by an ASCII digit.
    let bytes = lvl_text.as_bytes();
    let mut count: u8 = 0;
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'%' && bytes[i + 1].is_ascii_digit() {
            count = count.saturating_add(1);
            i += 2;
        } else {
            i += 1;
        }
    }
    count
}

/// Maps a [`DocxLevel`] to a [`ListLevel`]. `numbering` supplies the resolved
/// picture-bullet images (feature 5.4).
fn map_level(lvl: &DocxLevel, start_override: Option<u32>, numbering: &DocxNumbering) -> ListLevel {
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

    let kind = map_level_kind(lvl, start_value, &char_props, numbering);

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
    numbering: &DocxNumbering,
) -> ListLevelKind {
    let num_fmt = lvl.num_fmt.as_deref().unwrap_or("decimal");
    let lvl_text = lvl.lvl_text.as_deref();
    let font = char_props.font_name.clone();

    // A picture bullet wins over the text bullet char / numbering format when the
    // level references a `w:numPicBullet` whose image the importer resolved.
    if let Some(id) = lvl.lvl_pic_bullet_id
        && let Some(src) = numbering.pic_bullet_src(id)
    {
        return ListLevelKind::Bullet {
            char: BulletChar::Image {
                src: src.to_string(),
            },
            font,
        };
    }

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

// ── Public entry point ───────────────────────────────────────────────────────

/// Populates `catalog.list_styles` from a [`DocxNumbering`] definition.
///
/// For each `w:num` instance, resolves the backing `w:abstractNum` and
/// applies any `w:lvlOverride` entries, then maps all 9 levels to
/// [`ListLevel`] values.
///
/// Returns non-fatal [`OoxmlWarning`]s for unresolvable `abstractNumId`
/// references (corrupt or truncated files).
pub(crate) fn map_numbering(
    numbering: &DocxNumbering,
    catalog: &mut StyleCatalog,
) -> Vec<OoxmlWarning> {
    let mut warnings = Vec::new();

    for num in &numbering.nums {
        let Some(abs) = numbering
            .abstract_nums
            .iter()
            .find(|a| a.abstract_num_id == num.abstract_num_id)
        else {
            warnings.push(OoxmlWarning::UnresolvedNumberingId { num_id: num.num_id });
            continue;
        };

        // Build 9 levels (0..=8), applying overrides where present.
        let mut levels = Vec::with_capacity(9);
        for ilvl in 0u8..9 {
            // Check for a full level override first.
            let override_entry = num.level_overrides.iter().find(|o| o.ilvl == ilvl);

            if let Some(ov) = override_entry {
                if let Some(ref ov_lvl) = ov.level {
                    // Full level override supplied.
                    levels.push(map_level(ov_lvl, ov.start_override, numbering));
                    continue;
                }
                // Only a start override — use the abstract level with the new start.
                if let Some(base) = abs.levels.iter().find(|l| l.ilvl == ilvl) {
                    levels.push(map_level(base, ov.start_override, numbering));
                    continue;
                }
            }

            // No override: use the abstract level as-is.
            if let Some(base) = abs.levels.iter().find(|l| l.ilvl == ilvl) {
                levels.push(map_level(base, None, numbering));
            }
        }

        let list_id = ListId::new(num.num_id.to_string());
        let list_style = ListStyle {
            id: list_id.clone(),
            display_name: None,
            levels,
            extensions: ExtensionBag::default(),
        };
        catalog.list_styles.insert(list_id, list_style);
    }

    warnings
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "numbering_tests.rs"]
mod tests;
