// SPDX-License-Identifier: Apache-2.0

//! Inspector-row model (Spec 05 M2): every applicable property appears with its
//! resolved value and provenance — the fix for the old panel's local-only
//! blindness — and inherited rows name their source ancestor by display name.

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

fn named(mut s: ParagraphStyle, display: &str) -> ParagraphStyle {
    s.display_name = Some(display.to_string());
    s
}

fn insert(cat: &mut StyleCatalog, s: ParagraphStyle) {
    cat.paragraph_styles.insert(s.id.clone(), s);
}

fn row<'a>(rows: &'a [InspectorRow], p: StyleProperty) -> &'a InspectorRow {
    rows.iter()
        .find(|r| r.property == p)
        .unwrap_or_else(|| panic!("missing row for {p:?}"))
}

#[test]
fn lists_every_applicable_property_even_when_unset() {
    let cat = {
        let mut c = StyleCatalog::new();
        insert(
            &mut c,
            para("Body", None, ParaProps::default(), CharProps::default()),
        );
        c
    };
    let rows = paragraph_inspector_rows(&cat, &StyleId::new("Body"));
    // All 11 modelled properties appear, not just the (zero) locally-set ones —
    // this is the fix for the local-only blindness.
    assert_eq!(rows.len(), 11);
    // A wholly unset property is FormatDefault with no value.
    let a = row(&rows, StyleProperty::Alignment);
    assert_eq!(a.provenance, RowProvenance::FormatDefault);
    assert_eq!(a.value_display, None);
}

#[test]
fn local_property_is_marked_local_with_value() {
    let cat = {
        let mut c = StyleCatalog::new();
        let pp = ParaProps {
            alignment: Some(ParagraphAlignment::Center),
            ..Default::default()
        };
        insert(&mut c, para("Body", None, pp, CharProps::default()));
        c
    };
    let rows = paragraph_inspector_rows(&cat, &StyleId::new("Body"));
    let a = row(&rows, StyleProperty::Alignment);
    assert!(a.provenance.is_local());
    assert_eq!(a.value_display.as_deref(), Some("Center"));
}

#[test]
fn inherited_row_names_its_ancestor_by_display_name() {
    let cat = {
        let mut c = StyleCatalog::new();
        let base_pp = ParaProps {
            alignment: Some(ParagraphAlignment::Justify),
            ..Default::default()
        };
        // The ancestor has id "Heading1" but display name "Heading 1".
        insert(
            &mut c,
            named(
                para("Heading1", None, base_pp, CharProps::default()),
                "Heading 1",
            ),
        );
        insert(
            &mut c,
            para(
                "Subhead",
                Some("Heading1"),
                ParaProps::default(),
                CharProps::default(),
            ),
        );
        c
    };
    let rows = paragraph_inspector_rows(&cat, &StyleId::new("Subhead"));
    let a = row(&rows, StyleProperty::Alignment);
    match &a.provenance {
        RowProvenance::Inherited {
            ancestor_id,
            ancestor_display,
        } => {
            assert_eq!(ancestor_id, &StyleId::new("Heading1")); // stable id for jump-to-ancestor
            assert_eq!(ancestor_display, "Heading 1"); // display name for the UI
        }
        other => panic!("expected Inherited, got {other:?}"),
    }
    assert_eq!(a.value_display.as_deref(), Some("Justify"));
    assert!(!a.provenance.is_local());
}

#[test]
fn inherited_ancestor_without_display_name_falls_back_to_id() {
    let cat = {
        let mut c = StyleCatalog::new();
        let base_cp = CharProps {
            bold: Some(true),
            ..Default::default()
        };
        insert(
            &mut c,
            para("Strongish", None, ParaProps::default(), base_cp),
        ); // no display_name
        insert(
            &mut c,
            para(
                "Child",
                Some("Strongish"),
                ParaProps::default(),
                CharProps::default(),
            ),
        );
        c
    };
    let rows = paragraph_inspector_rows(&cat, &StyleId::new("Child"));
    let b = row(&rows, StyleProperty::Bold);
    match &b.provenance {
        RowProvenance::Inherited {
            ancestor_display, ..
        } => {
            assert_eq!(ancestor_display, "Strongish"); // fell back to the id
        }
        other => panic!("expected Inherited, got {other:?}"),
    }
    assert_eq!(b.value_display.as_deref(), Some("On"));
}

#[test]
fn document_default_property_is_marked_default() {
    let cat = {
        let mut c = StyleCatalog::new();
        let normal_pp = ParaProps {
            alignment: Some(ParagraphAlignment::Left),
            ..Default::default()
        };
        insert(
            &mut c,
            para("Normal", None, normal_pp, CharProps::default()),
        );
        insert(
            &mut c,
            para("Loose", None, ParaProps::default(), CharProps::default()),
        );
        c.default_paragraph_style = Some(StyleId::new("Normal"));
        c
    };
    let rows = paragraph_inspector_rows(&cat, &StyleId::new("Loose"));
    let a = row(&rows, StyleProperty::Alignment);
    assert_eq!(a.provenance, RowProvenance::Default);
    assert_eq!(a.value_display.as_deref(), Some("Left"));
}

#[test]
fn formats_points_and_run_default_char_props() {
    let cat = {
        let mut c = StyleCatalog::new();
        let pp = ParaProps {
            indent_start: Some(Points::new(36.0)),
            ..Default::default()
        };
        let cp = CharProps {
            font_size: Some(Points::new(12.0)),
            font_name: Some("Inter".to_string()),
            ..Default::default()
        };
        insert(&mut c, para("Body", None, pp, cp));
        c
    };
    let rows = paragraph_inspector_rows(&cat, &StyleId::new("Body"));
    assert_eq!(
        row(&rows, StyleProperty::IndentStart)
            .value_display
            .as_deref(),
        Some("36 pt")
    );
    assert_eq!(
        row(&rows, StyleProperty::FontSize).value_display.as_deref(),
        Some("12 pt")
    );
    assert_eq!(
        row(&rows, StyleProperty::FontFamily)
            .value_display
            .as_deref(),
        Some("Inter")
    );
}

#[test]
fn unknown_style_yields_no_rows() {
    let cat = StyleCatalog::new();
    assert!(paragraph_inspector_rows(&cat, &StyleId::new("Ghost")).is_empty());
}

#[test]
fn clearing_a_local_override_falls_through_to_inherited() {
    let mut cat = StyleCatalog::new();
    let base_pp = ParaProps {
        alignment: Some(ParagraphAlignment::Justify),
        ..Default::default()
    };
    insert(&mut cat, para("Base", None, base_pp, CharProps::default()));
    // Child locally overrides alignment to Center.
    let child_pp = ParaProps {
        alignment: Some(ParagraphAlignment::Center),
        ..Default::default()
    };
    insert(
        &mut cat,
        para("Child", Some("Base"), child_pp, CharProps::default()),
    );

    // Before reset: the row is Local/Center.
    let before = paragraph_inspector_rows(&cat, &StyleId::new("Child"));
    assert!(row(&before, StyleProperty::Alignment).provenance.is_local());

    // Reset the local override.
    let child = cat
        .paragraph_styles
        .get_mut(&StyleId::new("Child"))
        .unwrap();
    clear_local_property(child, StyleProperty::Alignment);

    // After reset: alignment now resolves as Inherited from Base (Justify).
    let after = paragraph_inspector_rows(&cat, &StyleId::new("Child"));
    let a = row(&after, StyleProperty::Alignment);
    assert_eq!(
        a.provenance,
        RowProvenance::Inherited {
            ancestor_id: StyleId::new("Base"),
            ancestor_display: "Base".to_string(),
        }
    );
    assert_eq!(a.value_display.as_deref(), Some("Justify"));
}

#[test]
fn clearing_the_only_source_falls_through_to_format_default() {
    let mut cat = StyleCatalog::new();
    let pp = ParaProps {
        indent_start: Some(loki_doc_model::loki_primitives::units::Points::new(24.0)),
        ..Default::default()
    };
    insert(&mut cat, para("Solo", None, pp, CharProps::default()));

    let solo = cat.paragraph_styles.get_mut(&StyleId::new("Solo")).unwrap();
    clear_local_property(solo, StyleProperty::IndentStart);

    let rows = paragraph_inspector_rows(&cat, &StyleId::new("Solo"));
    let r = row(&rows, StyleProperty::IndentStart);
    assert_eq!(r.provenance, RowProvenance::FormatDefault);
    assert_eq!(r.value_display, None);
}

#[test]
fn clearing_char_props_resets_run_defaults() {
    let mut cat = StyleCatalog::new();
    let cp = CharProps {
        bold: Some(true),
        font_name: Some("Inter".to_string()),
        ..Default::default()
    };
    insert(&mut cat, para("Body", None, ParaProps::default(), cp));

    let body = cat.paragraph_styles.get_mut(&StyleId::new("Body")).unwrap();
    clear_local_property(body, StyleProperty::Bold);
    clear_local_property(body, StyleProperty::FontFamily);

    let rows = paragraph_inspector_rows(&cat, &StyleId::new("Body"));
    assert_eq!(
        row(&rows, StyleProperty::Bold).provenance,
        RowProvenance::FormatDefault
    );
    assert_eq!(
        row(&rows, StyleProperty::FontFamily).provenance,
        RowProvenance::FormatDefault
    );
}
