// SPDX-License-Identifier: Apache-2.0

//! The list-level draft round trip: snapshot → edit → parse-back, the
//! decline on junk numerics, and the pass-through of unexposed fields.

use loki_doc_model::style::list_defaults::{
    default_bullet_list_style, default_numbered_list_style,
};
use loki_doc_model::style::{BulletChar, ListLevelKind, NumberingScheme};

use super::{DraftKind, ListLevelDraft};

#[test]
fn snapshot_edit_parse_back_round_trips() {
    let style = default_numbered_list_style();
    let level1 = &style.levels[1];
    let mut draft = ListLevelDraft::from_level(style.id.as_str(), level1);
    assert_eq!(draft.kind, DraftKind::Numbered);
    assert_eq!(draft.level, 1);

    draft.scheme = NumberingScheme::UpperRoman;
    draft.start = "4".into();
    draft.indent = "50".into();

    let parsed = draft.to_level(level1).expect("valid edit parses");
    let ListLevelKind::Numbered {
        scheme,
        start_value,
        display_levels,
        ..
    } = &parsed.kind
    else {
        panic!("still numbered");
    };
    assert_eq!(*scheme, NumberingScheme::UpperRoman);
    assert_eq!(*start_value, 4);
    // Unexposed field carried over from the existing level, not reset.
    let ListLevelKind::Numbered {
        display_levels: orig,
        ..
    } = &level1.kind
    else {
        panic!()
    };
    assert_eq!(display_levels, orig, "display_levels must pass through");
    assert!((parsed.indent_start.value() - 50.0).abs() < 0.01);
    assert_eq!(
        parsed.label_alignment, level1.label_alignment,
        "alignment passes through"
    );
}

#[test]
fn kind_switch_and_junk_numerics() {
    let style = default_bullet_list_style();
    let level0 = &style.levels[0];
    let mut draft = ListLevelDraft::from_level(style.id.as_str(), level0);
    assert_eq!(draft.kind, DraftKind::Bullet);

    // Bullet → numbered switch.
    draft.kind = DraftKind::Numbered;
    draft.start = "1".into();
    let parsed = draft.to_level(level0).expect("switch parses");
    assert!(matches!(parsed.kind, ListLevelKind::Numbered { .. }));

    // Junk numerics decline instead of writing.
    draft.start = "many".into();
    assert!(draft.to_level(level0).is_none(), "junk start must decline");
    draft.start = "1".into();
    draft.indent = "-5".into();
    assert!(
        draft.to_level(level0).is_none(),
        "negative indent must decline"
    );

    // Empty bullet char falls back to the default glyph.
    draft.kind = DraftKind::Bullet;
    draft.indent = "10".into();
    draft.bullet_char = String::new();
    let parsed = draft.to_level(level0).expect("empty char is defaulted");
    assert!(matches!(
        parsed.kind,
        ListLevelKind::Bullet {
            char: BulletChar::Char('\u{2022}'),
            ..
        }
    ));
}
