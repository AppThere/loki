// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Paragraph-style hierarchy queries + impact preview (Spec 05 M4).

use super::*;
use crate::style::props::char_props::CharProps;
use crate::style::props::para_props::{ParaProps, ParagraphAlignment};

fn para(id: &str, parent: Option<&str>, pp: ParaProps) -> ParagraphStyle {
    ParagraphStyle {
        id: StyleId::new(id),
        display_name: None,
        parent: parent.map(StyleId::new),
        linked_char_style: None,
        next_style_id: None,
        para_props: pp,
        char_props: CharProps::default(),
        is_default: false,
        is_custom: false,
        extensions: Default::default(),
    }
}

fn aligned(a: ParagraphAlignment) -> ParaProps {
    ParaProps {
        alignment: Some(a),
        ..Default::default()
    }
}

fn insert(cat: &mut StyleCatalog, s: ParagraphStyle) {
    cat.paragraph_styles.insert(s.id.clone(), s);
}

/// A → { B → { D }, C }
fn tree_catalog() -> StyleCatalog {
    let mut c = StyleCatalog::new();
    insert(&mut c, para("A", None, ParaProps::default()));
    insert(&mut c, para("B", Some("A"), ParaProps::default()));
    insert(&mut c, para("C", Some("A"), ParaProps::default()));
    insert(&mut c, para("D", Some("B"), ParaProps::default()));
    c
}

fn ids(v: &[StyleId]) -> Vec<&str> {
    v.iter().map(StyleId::as_str).collect()
}

#[test]
fn children_lists_direct_descendants_only() {
    let c = tree_catalog();
    assert_eq!(ids(&c.para_children(&StyleId::new("A"))), vec!["B", "C"]);
    assert_eq!(ids(&c.para_children(&StyleId::new("B"))), vec!["D"]);
    assert!(c.para_children(&StyleId::new("D")).is_empty());
}

#[test]
fn breadcrumb_is_root_first_including_self() {
    let c = tree_catalog();
    // D's path from the root: A → B → D.
    assert_eq!(
        ids(&c.para_breadcrumb(&StyleId::new("D"))),
        vec!["A", "B", "D"]
    );
    // A root style's breadcrumb is just itself.
    assert_eq!(ids(&c.para_breadcrumb(&StyleId::new("A"))), vec!["A"]);
}

#[test]
fn descendants_are_transitive_breadth_first() {
    let c = tree_catalog();
    assert_eq!(
        ids(&c.para_descendants(&StyleId::new("A"))),
        vec!["B", "C", "D"]
    );
    assert_eq!(ids(&c.para_descendants(&StyleId::new("B"))), vec!["D"]);
    assert!(c.para_descendants(&StyleId::new("D")).is_empty());
}

#[test]
fn descendants_terminate_on_a_cycle() {
    let mut c = StyleCatalog::new();
    // A → B → A (corrupt). Walk must not loop.
    insert(&mut c, para("A", Some("B"), ParaProps::default()));
    insert(&mut c, para("B", Some("A"), ParaProps::default()));
    let d = c.para_descendants(&StyleId::new("A"));
    // B is a descendant; A is not listed again (cycle guard).
    assert_eq!(ids(&d), vec!["B"]);
}

fn sets_alignment(s: &ParagraphStyle) -> bool {
    s.para_props.alignment.is_some()
}

#[test]
fn impact_preview_lists_dependents_that_inherit_the_property() {
    // A sets alignment; B & D inherit it; C overrides it locally.
    let mut c = StyleCatalog::new();
    insert(&mut c, para("A", None, aligned(ParagraphAlignment::Left)));
    insert(&mut c, para("B", Some("A"), ParaProps::default()));
    insert(
        &mut c,
        para("C", Some("A"), aligned(ParagraphAlignment::Right)),
    );
    insert(&mut c, para("D", Some("B"), ParaProps::default()));

    // Changing alignment on A affects B and D, not C (which overrides).
    let affected = c.dependents_affected(&StyleId::new("A"), sets_alignment);
    assert_eq!(ids(&affected), vec!["B", "D"]);
}

#[test]
fn impact_preview_excludes_subtrees_shadowed_by_a_closer_override() {
    // A sets alignment; B overrides it; D is under B.
    let mut c = StyleCatalog::new();
    insert(&mut c, para("A", None, aligned(ParagraphAlignment::Left)));
    insert(
        &mut c,
        para("B", Some("A"), aligned(ParagraphAlignment::Center)),
    );
    insert(&mut c, para("D", Some("B"), ParaProps::default()));

    // B overrides, so D inherits from B, not A — changing A affects neither.
    let affected = c.dependents_affected(&StyleId::new("A"), sets_alignment);
    assert!(
        affected.is_empty(),
        "shadowed subtree must be excluded: {:?}",
        ids(&affected)
    );
}

#[test]
fn impact_preview_covers_adding_a_new_override() {
    // A does not set alignment yet; B & D inherit from further up / engine.
    // Adding alignment at A would newly apply to B and D (no closer override).
    let mut c = StyleCatalog::new();
    insert(&mut c, para("A", None, ParaProps::default()));
    insert(&mut c, para("B", Some("A"), ParaProps::default()));
    insert(&mut c, para("D", Some("B"), ParaProps::default()));

    let affected = c.dependents_affected(&StyleId::new("A"), sets_alignment);
    assert_eq!(ids(&affected), vec!["B", "D"]);
}

#[test]
fn forest_preorder_lists_parents_before_subtrees_with_depths() {
    let c = tree_catalog();
    let order = c.para_forest_preorder();
    let flat: Vec<(&str, usize)> = order.iter().map(|(id, d)| (id.as_str(), *d)).collect();
    // A(0) then its subtree B(1) → D(2), then C(1).
    assert_eq!(flat, vec![("A", 0), ("B", 1), ("D", 2), ("C", 1)]);
}

#[test]
fn forest_preorder_surfaces_cycle_stranded_styles_once() {
    let mut c = StyleCatalog::new();
    // A → B → A: no root. Both must still appear exactly once.
    insert(&mut c, para("A", Some("B"), ParaProps::default()));
    insert(&mut c, para("B", Some("A"), ParaProps::default()));
    let order = c.para_forest_preorder();
    let mut ids: Vec<&str> = order.iter().map(|(id, _)| id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(ids, vec!["A", "B"]);
}

#[test]
fn reparent_cycle_guard_still_holds_for_the_tree_view() {
    // Sanity: the M1 guard composes with the downward view.
    let c = tree_catalog();
    assert!(c.para_reparent_cycles(&StyleId::new("A"), &StyleId::new("D")));
    assert!(!c.para_reparent_cycles(&StyleId::new("D"), &StyleId::new("C")));
}
