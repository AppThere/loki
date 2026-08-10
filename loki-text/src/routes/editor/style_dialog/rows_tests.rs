// SPDX-License-Identifier: Apache-2.0

//! Tests for the provenance-row mapping.

use super::*;
use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::style::ParagraphStyle;
use loki_doc_model::style::catalog::StyleId;

fn para(id: &str, parent: Option<&str>, display: &str) -> ParagraphStyle {
    ParagraphStyle {
        id: StyleId::new(id),
        display_name: Some(display.to_string()),
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

/// Default Paragraph Style → Body → Body indent, the design's worked example.
fn catalog() -> StyleCatalog {
    let mut cat = StyleCatalog::default();
    let mut root = para("default", None, "Default Paragraph Style");
    root.char_props.font_name = Some("Tinos".to_string());
    root.char_props.font_size = Some(Points::new(12.0));

    let mut body = para("body", Some("default"), "Body");
    body.char_props.bold = Some(true);

    let mut indent = para("body-indent", Some("body"), "Body indent");
    indent.para_props.indent_first_line = Some(Points::new(13.5));
    indent.char_props.bold = Some(false);

    for s in [root, body, indent] {
        cat.paragraph_styles.insert(s.id.clone(), s);
    }
    cat
}

/// Each model provenance maps to exactly one display kind, and the ancestor id
/// survives only where there is an ancestor to jump to.
#[test]
fn each_model_provenance_maps_to_its_own_display_kind() {
    let cat = catalog();
    assert_eq!(
        to_row_source(&cat, Provenance::Local).kind,
        AtProvenanceKind::Local
    );
    assert_eq!(
        to_row_source(&cat, Provenance::Default).kind,
        AtProvenanceKind::Default
    );
    assert_eq!(
        to_row_source(&cat, Provenance::FormatDefault).kind,
        AtProvenanceKind::Engine
    );

    let inherited = to_row_source(&cat, Provenance::Inherited(StyleId::new("body")));
    assert_eq!(inherited.kind, AtProvenanceKind::Inherited);
    assert_eq!(inherited.ancestor, Some(StyleId::new("body")));
    assert_eq!(inherited.ancestor_display.as_deref(), Some("Body"));
}

/// The line names the ancestor by its **display name** — the name the user sees
/// in the style list — falling back to the stable id only when there is none.
#[test]
fn an_unnamed_ancestor_falls_back_to_its_id() {
    let mut cat = catalog();
    if let Some(body) = cat.paragraph_styles.get_mut(&StyleId::new("body")) {
        body.display_name = None;
    }
    let source = to_row_source(&cat, Provenance::Inherited(StyleId::new("body")));
    assert_eq!(source.ancestor_display.as_deref(), Some("body"));
}

/// Only inherited rows offer the jump, and only local rows offer the reset —
/// offering both, or neither, on the same row is the mistake the affordances
/// exist to prevent.
#[test]
fn jump_and_reset_are_offered_on_disjoint_rows() {
    let cat = catalog();
    let local = to_row_source(&cat, Provenance::Local);
    assert!(local.reset_label().is_some());
    assert!(local.jump_label().is_none());

    let inherited = to_row_source(&cat, Provenance::Inherited(StyleId::new("body")));
    assert!(inherited.jump_label().is_some());
    assert!(inherited.reset_label().is_none());

    for p in [Provenance::Default, Provenance::FormatDefault] {
        let s = to_row_source(&cat, p.clone());
        assert!(s.jump_label().is_none(), "{p:?} has no ancestor to open");
        assert!(s.reset_label().is_none(), "{p:?} has no override to clear");
    }
}

/// A locally-set property resolves to Local with no jump; an inherited one
/// names the nearest ancestor that sets it, not the furthest.
#[test]
fn resolution_finds_the_nearest_ancestor_that_sets_the_property() {
    let cat = catalog();
    let id = StyleId::new("body-indent");

    let (source, value) = resolve_row(
        &cat,
        &id,
        |s| s.para_props.indent_first_line,
        |p| format!("{} pt", p.value()),
    )
    .expect("the style is in the catalog");
    assert_eq!(source.kind, AtProvenanceKind::Local);
    assert_eq!(value.as_deref(), Some("13.5 pt"));

    // font_name is set only on the root, two links up.
    let (source, value) = resolve_row(&cat, &id, |s| s.char_props.font_name.clone(), Clone::clone)
        .expect("the style is in the catalog");
    assert_eq!(source.kind, AtProvenanceKind::Inherited);
    assert_eq!(
        source.ancestor_display.as_deref(),
        Some("Default Paragraph Style")
    );
    assert_eq!(value.as_deref(), Some("Tinos"));
}

/// A local `false` must not be mistaken for "unset" — the bug that makes
/// "un-bolding a bold parent" look like it did nothing.
#[test]
fn a_locally_set_false_resolves_as_local_not_inherited() {
    let cat = catalog();
    let (source, value) = resolve_row(
        &cat,
        &StyleId::new("body-indent"),
        |s| s.char_props.bold,
        |b| b.to_string(),
    )
    .expect("the style is in the catalog");
    assert_eq!(source.kind, AtProvenanceKind::Local);
    assert_eq!(value.as_deref(), Some("false"));
}

/// An unknown style yields no row at all, so the caller renders the control
/// with no line rather than inventing a source.
#[test]
fn an_unknown_style_yields_no_row() {
    let cat = catalog();
    assert!(
        resolve_row(
            &cat,
            &StyleId::new("no-such-style"),
            |s| s.char_props.font_size,
            |p| p.value().to_string(),
        )
        .is_none()
    );
}

/// The two counters partition the property set, so the General tab's
/// "3 local · 8 inherited" line always sums to the whole.
#[test]
fn local_and_inherited_counts_partition_the_property_set() {
    let cat = catalog();
    for id in ["default", "body", "body-indent"] {
        let style = cat
            .paragraph_styles
            .get(&StyleId::new(id))
            .expect("seeded style");
        assert_eq!(
            local_property_count(style) + inherited_property_count(style),
            PROPERTY_COUNT,
            "{id} counts do not partition"
        );
    }
}

/// A style that sets nothing inherits everything; setting one property moves
/// exactly one across. Asserting only the empty case would pass for a counter
/// that always returned the total.
#[test]
fn setting_a_property_moves_exactly_one_from_inherited_to_local() {
    let mut style = para("x", None, "X");
    assert_eq!(inherited_property_count(&style), PROPERTY_COUNT);
    assert_eq!(local_property_count(&style), 0);

    style.para_props.indent_first_line = Some(Points::new(13.5));
    assert_eq!(inherited_property_count(&style), PROPERTY_COUNT - 1);
    assert_eq!(local_property_count(&style), 1);

    style.char_props.font_size = Some(Points::new(10.0));
    assert_eq!(local_property_count(&style), 2);
}
