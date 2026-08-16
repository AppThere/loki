// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`heading_style_id`](super::heading_style_id).

use super::heading_style_id;
use crate::content::attr::NodeAttr;

fn attr_with(style: &str) -> NodeAttr {
    let mut a = NodeAttr::default();
    a.kv.push(("style".to_string(), style.to_string()));
    a
}

#[test]
fn carried_style_name_wins_over_the_level() {
    // The whole point: an imported heading keeps its own style name, which
    // need not resemble "Heading1" at all.
    let sid = heading_style_id(1, &attr_with("Heading_20_1"));
    assert_eq!(sid.as_str(), "Heading_20_1");

    let sid = heading_style_id(2, &attr_with("SceneHeading"));
    assert_eq!(sid.as_str(), "SceneHeading");
}

#[test]
fn falls_back_to_the_canonical_name_when_no_style_is_carried() {
    // An in-app heading carries no attr; the canonical name is correct for it.
    for lvl in 1..=6u8 {
        let sid = heading_style_id(lvl, &NodeAttr::default());
        assert_eq!(sid.as_str(), format!("Heading{lvl}"));
    }
}

#[test]
fn the_fallback_clamps_out_of_range_levels() {
    // Levels above 6 have no named style; clamping keeps the reference
    // resolvable instead of emitting a dangling "Heading7".
    assert_eq!(
        heading_style_id(7, &NodeAttr::default()).as_str(),
        "Heading6"
    );
    assert_eq!(
        heading_style_id(255, &NodeAttr::default()).as_str(),
        "Heading6"
    );
    assert_eq!(
        heading_style_id(0, &NodeAttr::default()).as_str(),
        "Heading1"
    );
}

#[test]
fn an_unrelated_attr_key_does_not_satisfy_the_lookup() {
    // Guard inversion: only the "style" key carries the name. A heading with
    // other attrs (alignment, page break) must still take the fallback.
    let mut a = NodeAttr::default();
    a.kv.push(("jc".to_string(), "center".to_string()));
    a.kv.push(("page-break-before".to_string(), "true".to_string()));
    assert_eq!(heading_style_id(3, &a).as_str(), "Heading3");
}
