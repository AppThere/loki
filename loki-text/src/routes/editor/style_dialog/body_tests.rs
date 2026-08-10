// SPDX-License-Identifier: Apache-2.0

//! Tests for the header breadcrumb's chain walk.

use super::*;
use loki_doc_model::style::ParagraphStyle;
use loki_doc_model::style::catalog::StyleId;

fn para(id: &str, parent: Option<&str>, display: Option<&str>) -> ParagraphStyle {
    ParagraphStyle {
        id: StyleId::new(id),
        display_name: display.map(str::to_string),
        parent: parent.map(StyleId::new),
        linked_char_style: None,
        para_props: Default::default(),
        char_props: Default::default(),
        next_style_id: None,
        is_default: false,
        is_custom: true,
        extensions: Default::default(),
    }
}

fn catalog(styles: Vec<ParagraphStyle>) -> StyleCatalog {
    let mut cat = StyleCatalog::default();
    for s in styles {
        cat.paragraph_styles.insert(s.id.clone(), s);
    }
    cat
}

/// The design's worked example, root first.
#[test]
fn the_chain_reads_root_first_and_ends_at_the_edited_style() {
    let cat = catalog(vec![
        para("default", None, Some("Default Paragraph Style")),
        para("body", Some("default"), Some("Body")),
        para("body-indent", Some("body"), Some("Body indent")),
    ]);
    let names: Vec<String> = ancestry(&cat, &StyleId::new("body-indent"))
        .into_iter()
        .map(|(_, n)| n)
        .collect();
    assert_eq!(
        names,
        vec!["Default Paragraph Style", "Body", "Body indent"]
    );
}

/// A root style is its own whole chain — not an empty one, which would render a
/// blank breadcrumb under the title.
#[test]
fn a_root_style_is_its_own_chain() {
    let cat = catalog(vec![para("default", None, Some("Default"))]);
    assert_eq!(ancestry(&cat, &StyleId::new("default")).len(), 1);
}

/// An imported document can carry a self-referential or looping `basedOn`
/// chain. The walk must terminate — a spinning header is a hang, not a
/// cosmetic bug.
#[test]
fn a_cyclic_chain_terminates() {
    let cat = catalog(vec![
        para("a", Some("b"), Some("A")),
        para("b", Some("a"), Some("B")),
    ]);
    let chain = ancestry(&cat, &StyleId::new("a"));
    assert_eq!(chain.len(), 2, "each style appears once");

    let cat = catalog(vec![para("self", Some("self"), Some("Self"))]);
    assert_eq!(ancestry(&cat, &StyleId::new("self")).len(), 1);
}

/// A chain naming a parent that is not in the catalog stops there rather than
/// dropping the styles it already walked.
#[test]
fn a_dangling_parent_truncates_without_losing_the_walked_chain() {
    let cat = catalog(vec![para("body", Some("missing"), Some("Body"))]);
    let names: Vec<String> = ancestry(&cat, &StyleId::new("body"))
        .into_iter()
        .map(|(_, n)| n)
        .collect();
    assert_eq!(names, vec!["Body"]);
}

/// A style not in the catalog has no chain at all.
#[test]
fn an_unknown_style_has_no_chain() {
    let cat = catalog(vec![para("body", None, Some("Body"))]);
    assert!(ancestry(&cat, &StyleId::new("nope")).is_empty());
}

/// An unnamed style falls back to its stable id, so the breadcrumb never shows
/// a gap where a name should be.
#[test]
fn an_unnamed_style_falls_back_to_its_id() {
    let cat = catalog(vec![para("body-indent", None, None)]);
    let names: Vec<String> = ancestry(&cat, &StyleId::new("body-indent"))
        .into_iter()
        .map(|(_, n)| n)
        .collect();
    assert_eq!(names, vec!["body-indent"]);
}

/// The elision threshold must actually be reachable, and the edited style must
/// survive it — eliding the *last* entry would hide what is being edited.
#[test]
fn a_deep_chain_keeps_the_edited_style_at_the_end() {
    let mut styles = vec![para("s0", None, Some("s0"))];
    for i in 1..=BREADCRUMB_MAX + 2 {
        let parent = format!("s{}", i - 1);
        let id = format!("s{i}");
        styles.push(para(&id, Some(&parent), Some(&id)));
    }
    let deepest = format!("s{}", BREADCRUMB_MAX + 2);
    let chain = ancestry(&catalog(styles), &StyleId::new(&deepest));

    assert!(chain.len() > BREADCRUMB_MAX, "the threshold is reachable");
    assert_eq!(
        chain.last().map(|(_, n)| n.clone()),
        Some(deepest),
        "the edited style is the last entry"
    );
}
