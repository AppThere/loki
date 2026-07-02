// SPDX-License-Identifier: Apache-2.0

//! Impact preview (Spec 05 M4): the union of dependents affected by staged edits.

use super::*;
use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::style::ParagraphStyle;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::{ParaProps, ParagraphAlignment};

fn para(id: &str, parent: Option<&str>, pp: ParaProps, cp: CharProps) -> ParagraphStyle {
    ParagraphStyle {
        id: StyleId::new(id),
        display_name: None,
        parent: parent.map(StyleId::new),
        linked_char_style: None,
        next_style_id: None,
        para_props: pp,
        char_props: cp,
        is_default: false,
        is_custom: false,
        extensions: Default::default(),
    }
}

fn insert(cat: &mut StyleCatalog, s: ParagraphStyle) {
    cat.paragraph_styles.insert(s.id.clone(), s);
}

fn ids(v: &[StyleId]) -> Vec<&str> {
    v.iter().map(StyleId::as_str).collect()
}

/// A(align+size) → { B, C(align override), D under B }
fn catalog() -> StyleCatalog {
    let mut c = StyleCatalog::new();
    let a_pp = ParaProps {
        alignment: Some(ParagraphAlignment::Left),
        ..Default::default()
    };
    let a_cp = CharProps {
        font_size: Some(Points::new(12.0)),
        ..Default::default()
    };
    insert(&mut c, para("A", None, a_pp, a_cp));
    insert(
        &mut c,
        para("B", Some("A"), ParaProps::default(), CharProps::default()),
    );
    let c_pp = ParaProps {
        alignment: Some(ParagraphAlignment::Right),
        ..Default::default()
    };
    insert(&mut c, para("C", Some("A"), c_pp, CharProps::default()));
    insert(
        &mut c,
        para("D", Some("B"), ParaProps::default(), CharProps::default()),
    );
    c
}

#[test]
fn nothing_staged_means_no_impact() {
    let c = catalog();
    assert!(affected_dependents(&c, &StyleId::new("A"), &[]).is_empty());
}

#[test]
fn alignment_change_affects_inheriting_dependents_only() {
    let c = catalog();
    // C overrides alignment, so only B and D are affected.
    let affected = affected_dependents(&c, &StyleId::new("A"), &[StyleProperty::Alignment]);
    assert_eq!(ids(&affected), vec!["B", "D"]);
}

#[test]
fn multiple_changed_properties_are_unioned_without_duplicates() {
    let c = catalog();
    // FontSize is inherited by B, C, and D (none override it); alignment by B, D.
    // The union is B, C, D with no repeats.
    let affected = affected_dependents(
        &c,
        &StyleId::new("A"),
        &[StyleProperty::Alignment, StyleProperty::FontSize],
    );
    assert_eq!(ids(&affected), vec!["B", "D", "C"]);
}

#[test]
fn leaf_style_has_no_dependents() {
    let c = catalog();
    assert!(affected_dependents(&c, &StyleId::new("D"), &[StyleProperty::Alignment]).is_empty());
}
