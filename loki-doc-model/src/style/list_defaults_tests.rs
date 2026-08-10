// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The default list styles: full depth, correct kinds, and the ensure-if-absent
//! contract (a document's own definition under the same id must survive).

use super::{
    DEFAULT_BULLET_LIST_ID, DEFAULT_NUMBERED_LIST_ID, default_bullet_list_style,
    default_numbered_list_style, ensure_default_bullet, ensure_default_numbered,
};
use crate::StyleCatalog;
use crate::style::list_style::{ListLevelKind, NumberingScheme};

/// Both defaults define all nine levels — Tab promotion (tier 3) indexes the
/// level list directly, so a shorter definition would make deep Tab a panic
/// or a silent no-op.
#[test]
fn defaults_define_all_nine_levels() {
    for style in [default_bullet_list_style(), default_numbered_list_style()] {
        assert_eq!(style.levels.len(), 9);
        for (i, level) in style.levels.iter().enumerate() {
            assert_eq!(usize::from(level.level), i, "levels in order");
            assert!(level.indent_start.value() > 0.0);
        }
    }
}

#[test]
fn bullet_levels_are_bullets_and_numbered_levels_cycle_schemes() {
    let bullet = default_bullet_list_style();
    assert!(
        bullet
            .levels
            .iter()
            .all(|l| matches!(l.kind, ListLevelKind::Bullet { .. }))
    );

    let numbered = default_numbered_list_style();
    let schemes: Vec<_> = numbered
        .levels
        .iter()
        .map(|l| match &l.kind {
            ListLevelKind::Numbered { scheme, .. } => *scheme,
            other => panic!("numbered default contains a non-numbered level: {other:?}"),
        })
        .collect();
    assert_eq!(schemes[0], NumberingScheme::Decimal);
    assert_eq!(schemes[1], NumberingScheme::LowerAlpha);
    assert_eq!(schemes[2], NumberingScheme::LowerRoman);
    assert_eq!(schemes[3], NumberingScheme::Decimal, "cycle repeats");
}

/// `ensure_*` must not overwrite an existing definition — a round-tripped
/// document may carry its own (edited) style under the default id.
#[test]
fn ensure_keeps_an_existing_definition() {
    let mut catalog = StyleCatalog::default();
    let mut own = default_bullet_list_style();
    own.display_name = Some("Mine".to_string());
    catalog.list_styles.insert(own.id.clone(), own);

    let id = ensure_default_bullet(&mut catalog);
    assert_eq!(id.as_str(), DEFAULT_BULLET_LIST_ID);
    assert_eq!(
        catalog.list_styles[&id].display_name.as_deref(),
        Some("Mine"),
        "ensure overwrote the document's own definition"
    );

    // And on an empty catalog it inserts.
    let mut empty = StyleCatalog::default();
    let nid = ensure_default_numbered(&mut empty);
    assert_eq!(nid.as_str(), DEFAULT_NUMBERED_LIST_ID);
    assert!(empty.list_styles.contains_key(&nid));
}
