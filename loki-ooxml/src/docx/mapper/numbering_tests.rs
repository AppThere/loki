// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for `numbering`.

use super::*;
use crate::docx::model::numbering::{
    DocxAbstractNum, DocxLevel, DocxLvlOverride, DocxNum, DocxNumPicBullet,
};

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
        pic_bullets: Vec::new(),
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
        lvl_pic_bullet_id: None,
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
        lvl_pic_bullet_id: None,
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
fn picture_bullet_level_maps_to_image_bullet() {
    // A level referencing a numPicBullet whose image the importer resolved maps
    // to `BulletChar::Image { src }` (feature 5.4).
    let mut lvl = bullet_level(0, "•");
    lvl.lvl_pic_bullet_id = Some(0);
    let mut numbering = make_numbering(0, 1, vec![lvl], vec![]);
    numbering.pic_bullets.push(DocxNumPicBullet {
        id: 0,
        rel_id: "rId7".into(),
        src: Some("data:image/png;base64,AAAA".into()),
    });
    let mut catalog = StyleCatalog::new();
    map_numbering(&numbering, &mut catalog);
    let ls = catalog.list_styles.get(&ListId::new("1")).unwrap();
    let ListLevelKind::Bullet {
        char: BulletChar::Image { src },
        ..
    } = &ls.levels[0].kind
    else {
        panic!("expected an image bullet, got {:?}", ls.levels[0].kind);
    };
    assert_eq!(src, "data:image/png;base64,AAAA");
}

#[test]
fn picture_bullet_without_resolved_src_falls_back_to_text_bullet() {
    // If the image was not resolved (e.g. images not embedded), the level falls
    // back to its `w:numFmt="bullet"` text char rather than a blank bullet.
    let mut lvl = bullet_level(0, "•");
    lvl.lvl_pic_bullet_id = Some(0);
    let mut numbering = make_numbering(0, 1, vec![lvl], vec![]);
    numbering.pic_bullets.push(DocxNumPicBullet {
        id: 0,
        rel_id: "rId7".into(),
        src: None,
    });
    let mut catalog = StyleCatalog::new();
    map_numbering(&numbering, &mut catalog);
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
        pic_bullets: Vec::new(),
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
