// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `set_list_style_level`: the edit lands in the Loro catalog, the level
//! index cannot be forged, and absent targets decline without error.

use loro::LoroDoc;

use super::set_list_style_level;
use crate::document::Document;
use crate::loro_bridge::{document_to_loro, read_document_styles};
use crate::style::list_defaults::{DEFAULT_BULLET_LIST_ID, default_bullet_list_style};
use crate::style::list_style::{BulletChar, ListId, ListLevelKind, NumberingScheme};

fn loro_with_default_bullet() -> LoroDoc {
    let mut doc = Document::new();
    let style = default_bullet_list_style();
    doc.styles.list_styles.insert(style.id.clone(), style);
    document_to_loro(&doc).expect("to loro")
}

#[test]
fn edit_lands_and_level_index_is_forced() {
    let loro = loro_with_default_bullet();
    let base = read_document_styles(&loro);
    let mut lvl = base.list_styles[&ListId::new(DEFAULT_BULLET_LIST_ID)].levels[2].clone();

    // Turn level 2 into "start at 5" decimal numbering — and lie about the
    // index to prove it is forced back.
    lvl.kind = ListLevelKind::Numbered {
        scheme: NumberingScheme::Decimal,
        start_value: 5,
        format: "%3.".into(),
        display_levels: 1,
    };
    lvl.level = 7;
    set_list_style_level(&loro, DEFAULT_BULLET_LIST_ID, 2, lvl).expect("edit");

    let after = read_document_styles(&loro);
    let level2 = &after.list_styles[&ListId::new(DEFAULT_BULLET_LIST_ID)].levels[2];
    assert_eq!(level2.level, 2, "index must be forced to the slot");
    let ListLevelKind::Numbered {
        scheme,
        start_value,
        ..
    } = &level2.kind
    else {
        panic!("level 2 should now be numbered");
    };
    assert_eq!(*scheme, NumberingScheme::Decimal);
    assert_eq!(*start_value, 5);
    // Neighbours untouched.
    let level1 = &after.list_styles[&ListId::new(DEFAULT_BULLET_LIST_ID)].levels[1];
    assert!(matches!(
        level1.kind,
        ListLevelKind::Bullet {
            char: BulletChar::Char(_),
            ..
        }
    ));
}

#[test]
fn absent_style_and_absent_level_decline_without_error() {
    let loro = loro_with_default_bullet();
    let base = read_document_styles(&loro);
    let lvl = base.list_styles[&ListId::new(DEFAULT_BULLET_LIST_ID)].levels[0].clone();

    set_list_style_level(&loro, "no-such-style", 0, lvl.clone()).expect("unknown id is a no-op");
    set_list_style_level(&loro, DEFAULT_BULLET_LIST_ID, 200, lvl).expect("bad level is a no-op");

    // StyleCatalog is not PartialEq; the level-0 kind standing unchanged is
    // the discriminating probe (an accepted write would have replaced it).
    let after = read_document_styles(&loro);
    assert_eq!(
        after.list_styles[&ListId::new(DEFAULT_BULLET_LIST_ID)].levels,
        base.list_styles[&ListId::new(DEFAULT_BULLET_LIST_ID)].levels,
        "declined edits must not write"
    );
}
