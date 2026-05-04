// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

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

/// Maps a [`DocxLevel`] to a [`ListLevel`].
fn map_level(lvl: &DocxLevel, start_override: Option<u32>) -> ListLevel {
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

    let char_props = lvl
        .rpr
        .as_ref()
        .map(map_rpr)
        .unwrap_or_default();

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
                    // Take first Unicode scalar; fall back to bullet.
                    BulletChar::Char(s.chars().next().unwrap_or('•'))
                }
            };
            ListLevelKind::Bullet { char: bullet_char, font }
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
            .find(|a| a.abstract_num_id == num.abstract_num_id) else {
                warnings.push(OoxmlWarning::UnresolvedNumberingId {
                    num_id: num.num_id,
                });
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
                    levels.push(map_level(ov_lvl, ov.start_override));
                    continue;
                }
                // Only a start override — use the abstract level with the new start.
                if let Some(base) = abs.levels.iter().find(|l| l.ilvl == ilvl) {
                    levels.push(map_level(base, ov.start_override));
                    continue;
                }
            }

            // No override: use the abstract level as-is.
            if let Some(base) = abs.levels.iter().find(|l| l.ilvl == ilvl) {
                levels.push(map_level(base, None));
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
mod tests {
    use super::*;
    use crate::docx::model::numbering::{DocxAbstractNum, DocxLevel, DocxLvlOverride, DocxNum};

    fn make_numbering(
        abstract_num_id: u32,
        num_id: u32,
        levels: Vec<DocxLevel>,
        overrides: Vec<DocxLvlOverride>,
    ) -> DocxNumbering {
        DocxNumbering {
            abstract_nums: vec![DocxAbstractNum { abstract_num_id, levels }],
            nums: vec![DocxNum { num_id, abstract_num_id, level_overrides: overrides }],
        }
    }

    fn bullet_level(ilvl: u8, text: &str) -> DocxLevel {
        DocxLevel {
            ilvl,
            start: Some(1),
            num_fmt: Some("bullet".into()),
            lvl_text: Some(text.into()),
            lvl_jc: None,
            ppr: None,
            rpr: None,
        }
    }

    fn decimal_level(ilvl: u8, text: &str) -> DocxLevel {
        DocxLevel {
            ilvl,
            start: Some(1),
            num_fmt: Some("decimal".into()),
            lvl_text: Some(text.into()),
            lvl_jc: None,
            ppr: None,
            rpr: None,
        }
    }

    #[test]
    fn bullet_level_maps_correctly() {
        let numbering = make_numbering(0, 1, vec![bullet_level(0, "•")], vec![]);
        let mut catalog = StyleCatalog::new();
        let warnings = map_numbering(&numbering, &mut catalog);
        assert!(warnings.is_empty());
        let ls = catalog.list_styles.get(&ListId::new("1")).unwrap();
        assert!(matches!(
            ls.levels[0].kind,
            ListLevelKind::Bullet { char: BulletChar::Char('•'), .. }
        ));
    }

    #[test]
    fn decimal_level_maps_correctly() {
        let numbering = make_numbering(0, 1, vec![decimal_level(0, "%1.")], vec![]);
        let mut catalog = StyleCatalog::new();
        map_numbering(&numbering, &mut catalog);
        let ls = catalog.list_styles.get(&ListId::new("1")).unwrap();
        if let ListLevelKind::Numbered { scheme, format, .. } = &ls.levels[0].kind {
            assert_eq!(*scheme, NumberingScheme::Decimal);
            assert_eq!(format, "%1.");
        } else {
            panic!("expected Numbered");
        }
    }

    #[test]
    fn start_override_applied() {
        let numbering = make_numbering(
            0,
            1,
            vec![decimal_level(0, "%1.")],
            vec![DocxLvlOverride { ilvl: 0, start_override: Some(5), level: None }],
        );
        let mut catalog = StyleCatalog::new();
        map_numbering(&numbering, &mut catalog);
        let ls = catalog.list_styles.get(&ListId::new("1")).unwrap();
        if let ListLevelKind::Numbered { start_value, .. } = &ls.levels[0].kind {
            assert_eq!(*start_value, 5);
        } else {
            panic!("expected Numbered");
        }
    }

    #[test]
    fn unresolvable_abstract_num_produces_warning() {
        let numbering = DocxNumbering {
            abstract_nums: vec![],
            nums: vec![DocxNum {
                num_id: 99,
                abstract_num_id: 42,
                level_overrides: vec![],
            }],
        };
        let mut catalog = StyleCatalog::new();
        let warnings = map_numbering(&numbering, &mut catalog);
        assert!(!warnings.is_empty());
        assert!(catalog.list_styles.is_empty());
    }

    #[test]
    fn display_levels_counted_correctly() {
        assert_eq!(count_display_levels("%1.%2."), 2);
        assert_eq!(count_display_levels("%1."), 1);
        assert_eq!(count_display_levels("•"), 0);
    }
}
