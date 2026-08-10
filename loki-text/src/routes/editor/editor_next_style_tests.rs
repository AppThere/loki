// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the Enter-path next-style resolution: display keys must resolve
//! to catalog ids (the defect that made `next_style_id` dead for headings and
//! plain paragraphs), heading next styles must map to a type change, and
//! self-loops must be no-ops.

use loki_doc_model::style::{ParagraphStyle, StyleCatalog, StyleId};

use super::{NextBlock, next_block_for_split};

/// A catalog style `id` whose `next_style_id` is `next` (when given).
fn style(id: &str, next: Option<&str>) -> ParagraphStyle {
    ParagraphStyle {
        id: StyleId::new(id),
        display_name: None,
        parent: None,
        linked_char_style: None,
        next_style_id: next.map(String::from),
        para_props: Default::default(),
        char_props: Default::default(),
        is_default: false,
        is_custom: false,
        extensions: Default::default(),
    }
}

fn catalog(styles: Vec<ParagraphStyle>) -> StyleCatalog {
    let mut c = StyleCatalog::default();
    for s in styles {
        c.paragraph_styles.insert(s.id.clone(), s);
    }
    c
}

#[test]
fn heading_display_key_resolves_to_the_canonical_id() {
    // The bug this module exists for: get_block_style_name returns "Heading 1"
    // while the catalog holds "Heading1" — the old exact-match lookup found
    // nothing and next_style_id never fired.
    let c = catalog(vec![
        style("Heading1", Some("Normal")),
        style("Normal", None),
    ]);
    assert_eq!(
        next_block_for_split(&c, "Heading 1", None),
        Some(NextBlock::Styled("Normal".into()))
    );
}

#[test]
fn default_paragraph_style_key_follows_the_catalog_default() {
    let mut c = catalog(vec![style("Body", Some("Body2")), style("Body2", None)]);
    c.default_paragraph_style = Some(StyleId::new("Body"));
    assert_eq!(
        next_block_for_split(&c, "Default Paragraph Style", None),
        Some(NextBlock::Styled("Body2".into()))
    );
}

#[test]
fn default_key_without_a_catalog_default_resolves_nothing() {
    // Inverse of the above: no default registered → no style → no next.
    let c = catalog(vec![style("Body", Some("Body2"))]);
    assert_eq!(
        next_block_for_split(&c, "Default Paragraph Style", None),
        None
    );
}

#[test]
fn stored_heading_style_wins_over_the_canonical_id() {
    // The block's stored heading_style (ODF import) is what the layout
    // resolver consults, so its next style — not Heading1's — must apply.
    let c = catalog(vec![
        style("Heading_20_1", Some("Text_20_body")),
        style("Heading1", Some("Normal")),
        style("Text_20_body", None),
        style("Normal", None),
    ]);
    assert_eq!(
        next_block_for_split(&c, "Heading 1", Some("Heading_20_1".into())),
        Some(NextBlock::Styled("Text_20_body".into()))
    );
}

#[test]
fn a_next_style_naming_a_heading_maps_to_a_type_change() {
    let c = catalog(vec![style("Title", Some("Heading2"))]);
    assert_eq!(
        next_block_for_split(&c, "Title", None),
        Some(NextBlock::Heading(2))
    );
}

#[test]
fn a_display_form_heading_next_id_also_maps_to_a_type_change() {
    let c = catalog(vec![style("Title", Some("Heading 3"))]);
    assert_eq!(
        next_block_for_split(&c, "Title", None),
        Some(NextBlock::Heading(3))
    );
}

#[test]
fn a_heading_named_style_that_is_not_a_level_stays_styled() {
    // "HeadingBanner" is not Heading{1..6}; it must not be misread as a level.
    let c = catalog(vec![style("Title", Some("HeadingBanner"))]);
    assert_eq!(
        next_block_for_split(&c, "Title", None),
        Some(NextBlock::Styled("HeadingBanner".into()))
    );
}

#[test]
fn a_self_loop_is_a_no_op() {
    // Screenplay Dialogue → Dialogue: the tail already carries the style via
    // the split copy, so no mutation should be issued.
    let c = catalog(vec![style("Dialogue", Some("Dialogue"))]);
    assert_eq!(next_block_for_split(&c, "Dialogue", None), None);
}

#[test]
fn a_style_without_next_resolves_nothing() {
    let c = catalog(vec![style("Normal", None)]);
    assert_eq!(next_block_for_split(&c, "Normal", None), None);
}

#[test]
fn an_undefined_style_resolves_nothing() {
    let c = catalog(vec![]);
    assert_eq!(next_block_for_split(&c, "Ghost", None), None);
}
