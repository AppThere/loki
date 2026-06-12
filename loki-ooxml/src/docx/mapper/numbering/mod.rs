// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Numbering mapper: [`DocxNumbering`] → [`ListStyle`] entries in a [`StyleCatalog`].
//!
//! OOXML uses three-level indirection: paragraph `w:numId` → `w:num`
//! → `w:abstractNum`. Each `w:num` instance may override individual levels
//! via `w:lvlOverride`. ECMA-376 §17.9.

mod helpers;
mod level;

use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_doc_model::style::list_style::{ListId, ListStyle};

use crate::docx::model::numbering::DocxNumbering;
use crate::error::OoxmlWarning;

use level::map_level;

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
    use loki_doc_model::style::list_style::{BulletChar, ListLevelKind, NumberingScheme};

    use helpers::count_display_levels;

    fn make_numbering(
        abstract_num_id: u32,
        num_id: u32,
        levels: Vec<DocxLevel>,
        overrides: Vec<DocxLvlOverride>,
    ) -> DocxNumbering {
        DocxNumbering {
            abstract_nums: vec![DocxAbstractNum {
                abstract_num_id,
                levels,
            }],
            nums: vec![DocxNum {
                num_id,
                abstract_num_id,
                level_overrides: overrides,
            }],
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
            ListLevelKind::Bullet {
                char: BulletChar::Char('•'),
                ..
            }
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
            vec![DocxLvlOverride {
                ilvl: 0,
                start_override: Some(5),
                level: None,
            }],
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

    #[test]
    fn pua_wingdings_bullet_normalized_to_unicode() {
        // U+F0B7 is the Wingdings bullet (PUA); must be remapped to U+2022 •.
        let numbering = make_numbering(0, 1, vec![bullet_level(0, "\u{F0B7}")], vec![]);
        let mut catalog = StyleCatalog::new();
        map_numbering(&numbering, &mut catalog);
        let ls = catalog.list_styles.get(&ListId::new("1")).unwrap();
        assert!(
            matches!(
                ls.levels[0].kind,
                ListLevelKind::Bullet {
                    char: BulletChar::Char('•'),
                    ..
                }
            ),
            "U+F0B7 Wingdings bullet should normalize to U+2022 BULLET"
        );
    }

    #[test]
    fn pua_wingdings_square_normalized_to_unicode() {
        // U+F0FC is the Wingdings filled square; must remap to ■.
        let numbering = make_numbering(0, 1, vec![bullet_level(0, "\u{F0FC}")], vec![]);
        let mut catalog = StyleCatalog::new();
        map_numbering(&numbering, &mut catalog);
        let ls = catalog.list_styles.get(&ListId::new("1")).unwrap();
        assert!(matches!(
            ls.levels[0].kind,
            ListLevelKind::Bullet {
                char: BulletChar::Char('■'),
                ..
            }
        ));
    }

    #[test]
    fn standard_unicode_bullet_unchanged() {
        // Non-PUA Unicode bullets must not be remapped.
        for (ch, _desc) in [
            ('•', "bullet"),
            ('–', "en-dash"),
            ('○', "circle"),
            ('▪', "square"),
        ] {
            let numbering = make_numbering(0, 1, vec![bullet_level(0, &ch.to_string())], vec![]);
            let mut catalog = StyleCatalog::new();
            map_numbering(&numbering, &mut catalog);
            let ls = catalog.list_styles.get(&ListId::new("1")).unwrap();
            assert!(
                matches!(&ls.levels[0].kind, ListLevelKind::Bullet { char: BulletChar::Char(c), .. } if *c == ch),
                "Standard bullet char '{ch}' should not be remapped"
            );
        }
    }
}
