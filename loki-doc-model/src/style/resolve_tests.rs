// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Provenance-aware resolution (Spec 05 M1): Local / Inherited-from / Default /
//! FormatDefault over the single-parent tree, cycle safety, and the re-parent
//! cycle guard.

use super::*;
use crate::style::char_style::CharacterStyle;
use crate::style::para_style::ParagraphStyle;
use crate::style::props::char_props::CharProps;
use crate::style::props::para_props::{ParaProps, ParagraphAlignment};

// ── Builders ────────────────────────────────────────────────────────────────

fn para(id: &str, parent: Option<&str>, props: ParaProps, char_props: CharProps) -> ParagraphStyle {
    ParagraphStyle {
        id: StyleId::new(id),
        display_name: None,
        parent: parent.map(StyleId::new),
        linked_char_style: None,
        next_style_id: None,
        para_props: props,
        char_props,
        is_default: false,
        is_custom: false,
        extensions: Default::default(),
    }
}

fn char_style(id: &str, parent: Option<&str>, char_props: CharProps) -> CharacterStyle {
    CharacterStyle {
        id: StyleId::new(id),
        display_name: None,
        parent: parent.map(StyleId::new),
        char_props,
        extensions: Default::default(),
    }
}

fn aligned(a: ParagraphAlignment) -> ParaProps {
    ParaProps {
        alignment: Some(a),
        ..Default::default()
    }
}

fn bold() -> CharProps {
    CharProps {
        bold: Some(true),
        ..Default::default()
    }
}

fn insert_para(cat: &mut StyleCatalog, s: ParagraphStyle) {
    cat.paragraph_styles.insert(s.id.clone(), s);
}

fn align_of(s: &ParagraphStyle) -> Option<ParagraphAlignment> {
    s.para_props.alignment
}

// ── Paragraph provenance ─────────────────────────────────────────────────────

#[test]
fn local_property_resolves_as_local() {
    let mut cat = StyleCatalog::new();
    insert_para(
        &mut cat,
        para(
            "Body",
            None,
            aligned(ParagraphAlignment::Center),
            CharProps::default(),
        ),
    );

    let r = cat
        .resolve_para_chain(&StyleId::new("Body"), align_of)
        .unwrap();
    assert_eq!(r.provenance, Provenance::Local);
    assert_eq!(r.value, Some(ParagraphAlignment::Center));
    assert!(r.is_local());
}

#[test]
fn inherited_property_names_its_source_ancestor() {
    let mut cat = StyleCatalog::new();
    insert_para(
        &mut cat,
        para(
            "Base",
            None,
            aligned(ParagraphAlignment::Justify),
            CharProps::default(),
        ),
    );
    insert_para(
        &mut cat,
        para(
            "Mid",
            Some("Base"),
            ParaProps::default(),
            CharProps::default(),
        ),
    );
    insert_para(
        &mut cat,
        para(
            "Leaf",
            Some("Mid"),
            ParaProps::default(),
            CharProps::default(),
        ),
    );

    // Leaf and Mid set no alignment; it is inherited from Base two levels up.
    let r = cat
        .resolve_para_chain(&StyleId::new("Leaf"), align_of)
        .unwrap();
    assert_eq!(r.provenance, Provenance::Inherited(StyleId::new("Base")));
    assert_eq!(r.value, Some(ParagraphAlignment::Justify));
    assert!(!r.is_local());
}

#[test]
fn nearest_ancestor_wins_over_farther_one() {
    let mut cat = StyleCatalog::new();
    insert_para(
        &mut cat,
        para(
            "Base",
            None,
            aligned(ParagraphAlignment::Left),
            CharProps::default(),
        ),
    );
    insert_para(
        &mut cat,
        para(
            "Mid",
            Some("Base"),
            aligned(ParagraphAlignment::Right),
            CharProps::default(),
        ),
    );
    insert_para(
        &mut cat,
        para(
            "Leaf",
            Some("Mid"),
            ParaProps::default(),
            CharProps::default(),
        ),
    );

    let r = cat
        .resolve_para_chain(&StyleId::new("Leaf"), align_of)
        .unwrap();
    assert_eq!(r.provenance, Provenance::Inherited(StyleId::new("Mid")));
    assert_eq!(r.value, Some(ParagraphAlignment::Right));
}

#[test]
fn document_default_supplies_unset_property_as_default() {
    let mut cat = StyleCatalog::new();
    // "Normal" is the document default and sets alignment; "Loose" does not
    // chain to it, so the property falls through as Default (docDefaults).
    insert_para(
        &mut cat,
        para(
            "Normal",
            None,
            aligned(ParagraphAlignment::Left),
            CharProps::default(),
        ),
    );
    insert_para(
        &mut cat,
        para("Loose", None, ParaProps::default(), CharProps::default()),
    );
    cat.default_paragraph_style = Some(StyleId::new("Normal"));

    let r = cat
        .resolve_para_chain(&StyleId::new("Loose"), align_of)
        .unwrap();
    assert_eq!(r.provenance, Provenance::Default);
    assert_eq!(r.value, Some(ParagraphAlignment::Left));
}

#[test]
fn explicit_chain_to_default_reports_inherited_not_default() {
    let mut cat = StyleCatalog::new();
    // When a style explicitly bases on the default, a property set there is
    // *Inherited from that named ancestor*, not the anonymous Default level.
    insert_para(
        &mut cat,
        para(
            "Normal",
            None,
            aligned(ParagraphAlignment::Left),
            CharProps::default(),
        ),
    );
    insert_para(
        &mut cat,
        para(
            "Quote",
            Some("Normal"),
            ParaProps::default(),
            CharProps::default(),
        ),
    );
    cat.default_paragraph_style = Some(StyleId::new("Normal"));

    let r = cat
        .resolve_para_chain(&StyleId::new("Quote"), align_of)
        .unwrap();
    assert_eq!(r.provenance, Provenance::Inherited(StyleId::new("Normal")));
}

#[test]
fn unset_everywhere_is_format_default_with_no_value() {
    let mut cat = StyleCatalog::new();
    insert_para(
        &mut cat,
        para("Bare", None, ParaProps::default(), CharProps::default()),
    );

    let r = cat
        .resolve_para_chain(&StyleId::new("Bare"), align_of)
        .unwrap();
    assert_eq!(r.provenance, Provenance::FormatDefault);
    assert_eq!(r.value, None);
}

#[test]
fn run_default_char_prop_of_a_paragraph_style_resolves_via_same_method() {
    let mut cat = StyleCatalog::new();
    insert_para(&mut cat, para("Base", None, ParaProps::default(), bold()));
    insert_para(
        &mut cat,
        para(
            "Leaf",
            Some("Base"),
            ParaProps::default(),
            CharProps::default(),
        ),
    );

    // The same generic method resolves a paragraph style's run-default char prop.
    let r = cat
        .resolve_para_chain(&StyleId::new("Leaf"), |s| s.char_props.bold)
        .unwrap();
    assert_eq!(r.provenance, Provenance::Inherited(StyleId::new("Base")));
    assert_eq!(r.value, Some(true));
}

#[test]
fn unknown_style_id_resolves_to_none() {
    let cat = StyleCatalog::new();
    assert!(
        cat.resolve_para_chain(&StyleId::new("Ghost"), align_of)
            .is_none()
    );
}

#[test]
fn cyclic_parent_chain_terminates() {
    let mut cat = StyleCatalog::new();
    // A → B → A. No alignment set anywhere; resolution must terminate, not loop.
    insert_para(
        &mut cat,
        para("A", Some("B"), ParaProps::default(), CharProps::default()),
    );
    insert_para(
        &mut cat,
        para("B", Some("A"), ParaProps::default(), CharProps::default()),
    );

    let r = cat
        .resolve_para_chain(&StyleId::new("A"), align_of)
        .unwrap();
    assert_eq!(r.provenance, Provenance::FormatDefault);
}

// ── Character-style provenance (fixes the misnamed `resolve_char`) ────────────

#[test]
fn character_style_inherits_along_its_own_chain() {
    let mut cat = StyleCatalog::new();
    cat.character_styles.insert(
        StyleId::new("Emphasis"),
        char_style("Emphasis", None, bold()),
    );
    cat.character_styles.insert(
        StyleId::new("StrongEmphasis"),
        char_style("StrongEmphasis", Some("Emphasis"), CharProps::default()),
    );

    let r = cat
        .resolve_char_chain(&StyleId::new("StrongEmphasis"), |s| s.char_props.bold)
        .unwrap();
    assert_eq!(
        r.provenance,
        Provenance::Inherited(StyleId::new("Emphasis"))
    );
    assert_eq!(r.value, Some(true));
}

#[test]
fn character_style_unset_is_format_default() {
    let mut cat = StyleCatalog::new();
    cat.character_styles.insert(
        StyleId::new("Plain"),
        char_style("Plain", None, CharProps::default()),
    );

    let r = cat
        .resolve_char_chain(&StyleId::new("Plain"), |s| s.char_props.italic)
        .unwrap();
    assert_eq!(r.provenance, Provenance::FormatDefault);
    assert_eq!(r.value, None);
}

#[test]
fn character_style_falls_through_to_the_document_default_char_style() {
    // The docDefaults run defaults live in a synthetic default character style;
    // a property unset along the queried style's own chain resolves to it as
    // `Default` (ADR-0012 Decision 1 — the character family's `Default` source).
    let mut cat = StyleCatalog::new();
    cat.character_styles.insert(
        StyleId::new("__DocDefaultChar"),
        char_style("__DocDefaultChar", None, bold()),
    );
    cat.default_character_style = Some(StyleId::new("__DocDefaultChar"));
    cat.character_styles.insert(
        StyleId::new("Plain"),
        char_style("Plain", None, CharProps::default()),
    );

    let r = cat
        .resolve_char_chain(&StyleId::new("Plain"), |s| s.char_props.bold)
        .unwrap();
    assert_eq!(r.provenance, Provenance::Default);
    assert_eq!(r.value, Some(true));
}

#[test]
fn character_local_value_wins_over_the_document_default() {
    // A value set on the style itself is `Local`, never the doc default.
    let mut cat = StyleCatalog::new();
    cat.character_styles.insert(
        StyleId::new("__DocDefaultChar"),
        char_style("__DocDefaultChar", None, bold()),
    );
    cat.default_character_style = Some(StyleId::new("__DocDefaultChar"));
    let mut not_bold = CharProps::default();
    not_bold.bold = Some(false);
    cat.character_styles
        .insert(StyleId::new("Plain"), char_style("Plain", None, not_bold));

    let r = cat
        .resolve_char_chain(&StyleId::new("Plain"), |s| s.char_props.bold)
        .unwrap();
    assert_eq!(r.provenance, Provenance::Local);
    assert_eq!(r.value, Some(false));
}

// ── Re-parent cycle guard ────────────────────────────────────────────────────

#[test]
fn reparent_cycle_guard_rejects_self_and_descendants() {
    let mut cat = StyleCatalog::new();
    insert_para(
        &mut cat,
        para("A", None, ParaProps::default(), CharProps::default()),
    );
    insert_para(
        &mut cat,
        para("B", Some("A"), ParaProps::default(), CharProps::default()),
    );
    insert_para(
        &mut cat,
        para("C", Some("B"), ParaProps::default(), CharProps::default()),
    );
    insert_para(
        &mut cat,
        para("Other", None, ParaProps::default(), CharProps::default()),
    );

    let a = StyleId::new("A");
    let b = StyleId::new("B");
    let c = StyleId::new("C");
    let other = StyleId::new("Other");

    // Self-parent is a cycle.
    assert!(cat.para_reparent_cycles(&a, &a));
    // Re-parenting A under its descendant C is a cycle.
    assert!(cat.para_reparent_cycles(&a, &c));
    // Re-parenting A under its direct child B is a cycle.
    assert!(cat.para_reparent_cycles(&a, &b));
    // Re-parenting C under an unrelated style is fine.
    assert!(!cat.para_reparent_cycles(&c, &other));
    // Re-parenting C under A (its grandparent — already an ancestor) is fine:
    // it stays a tree.
    assert!(!cat.para_reparent_cycles(&c, &a));
}

#[test]
fn para_ancestors_lists_chain_nearest_first_including_self() {
    let mut cat = StyleCatalog::new();
    insert_para(
        &mut cat,
        para("A", None, ParaProps::default(), CharProps::default()),
    );
    insert_para(
        &mut cat,
        para("B", Some("A"), ParaProps::default(), CharProps::default()),
    );
    insert_para(
        &mut cat,
        para("C", Some("B"), ParaProps::default(), CharProps::default()),
    );

    assert_eq!(
        cat.para_ancestors(&StyleId::new("C")),
        vec![StyleId::new("C"), StyleId::new("B"), StyleId::new("A")]
    );
}
