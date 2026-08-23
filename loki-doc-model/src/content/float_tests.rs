// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for [`super`]: float wrap/side/placement round-tripping
//! through a `NodeAttr`.

use super::*;

#[test]
fn store_then_read_round_trips() {
    let mut attr = NodeAttr::default();
    let fw = FloatWrap {
        wrap: TextWrap::Tight,
        side: WrapSide::Left,
        align: None,
        behind_text: false,
        dist: None,
    };
    fw.store(&mut attr);
    assert!(attr.classes.iter().any(|c| c == FLOATING_CLASS));
    assert_eq!(FloatWrap::read(&attr), Some(fw));
}

#[test]
fn class_only_attr_reads_as_default_float() {
    // An anchored image tagged floating but with no wrap keys (e.g. a DOCX
    // `wp:anchor` with no wrap child) is floating, not inline.
    let mut attr = NodeAttr::default();
    attr.classes.push(FLOATING_CLASS.to_string());
    assert_eq!(FloatWrap::read(&attr), None, "no wrap keys → read is None");
    let fw = FloatWrap::read_or_class_default(&attr).expect("class marks it floating");
    assert_eq!(fw.wrap, TextWrap::Square);
    assert_eq!(fw.side, WrapSide::Both);
    assert!(!fw.behind_text);
}

#[test]
fn inline_attr_reads_or_class_default_is_none() {
    // No wrap keys and no floating class → genuinely inline.
    let attr = NodeAttr::default();
    assert_eq!(FloatWrap::read_or_class_default(&attr), None);
}

#[test]
fn explicit_wrap_wins_over_class_default() {
    // When wrap keys are present, the stored config is returned verbatim.
    let mut attr = NodeAttr::default();
    let fw = FloatWrap {
        wrap: TextWrap::Tight,
        side: WrapSide::Left,
        align: None,
        behind_text: false,
        dist: None,
    };
    fw.store(&mut attr);
    assert_eq!(FloatWrap::read_or_class_default(&attr), Some(fw));
}

#[test]
fn behind_text_round_trips() {
    let mut attr = NodeAttr::default();
    let fw = FloatWrap {
        wrap: TextWrap::None,
        side: WrapSide::Both,
        align: None,
        behind_text: true,
        dist: None,
    };
    fw.store(&mut attr);
    assert_eq!(FloatWrap::read(&attr), Some(fw));
}

#[test]
fn store_is_idempotent() {
    let mut attr = NodeAttr::default();
    FloatWrap {
        wrap: TextWrap::Square,
        side: WrapSide::Both,
        align: None,
        behind_text: false,
        dist: None,
    }
    .store(&mut attr);
    FloatWrap {
        wrap: TextWrap::Through,
        side: WrapSide::Right,
        align: None,
        behind_text: false,
        dist: None,
    }
    .store(&mut attr);
    // Only one floating class, one set of wrap keys.
    assert_eq!(
        attr.classes.iter().filter(|c| *c == FLOATING_CLASS).count(),
        1
    );
    assert_eq!(attr.kv.iter().filter(|(k, _)| k == KV_WRAP).count(), 1);
    assert_eq!(
        FloatWrap::read(&attr),
        Some(FloatWrap {
            wrap: TextWrap::Through,
            side: WrapSide::Right,
            align: None,
            behind_text: false,
            dist: None,
        })
    );
}

#[test]
fn read_none_when_absent() {
    assert_eq!(FloatWrap::read(&NodeAttr::default()), None);
}
