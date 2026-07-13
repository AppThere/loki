// SPDX-License-Identifier: Apache-2.0

//! Character-style inspector rows (Spec 05 M6): every character property with
//! provenance, resolved over the character family's own inheritance chain.

use super::*;
use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::style::CharacterStyle;
use loki_doc_model::style::props::char_props::CharProps;

fn char_style(id: &str, parent: Option<&str>, cp: CharProps) -> CharacterStyle {
    CharacterStyle {
        id: StyleId::new(id),
        display_name: None,
        parent: parent.map(StyleId::new),
        char_props: cp,
        extensions: Default::default(),
    }
}

fn named(mut s: CharacterStyle, display: &str) -> CharacterStyle {
    s.display_name = Some(display.to_string());
    s
}

fn insert(cat: &mut StyleCatalog, s: CharacterStyle) {
    cat.character_styles.insert(s.id.clone(), s);
}

fn row(rows: &[InspectorRow], p: StyleProperty) -> &InspectorRow {
    rows.iter()
        .find(|r| r.property == p)
        .unwrap_or_else(|| panic!("missing row for {p:?}"))
}

#[test]
fn lists_the_four_character_properties() {
    let mut cat = StyleCatalog::new();
    insert(&mut cat, char_style("Plain", None, CharProps::default()));
    let rows = character_inspector_rows(&cat, &StyleId::new("Plain"));
    assert_eq!(rows.len(), 4);
    // A wholly unset property is FormatDefault with no value.
    let bold = row(&rows, StyleProperty::Bold);
    assert_eq!(bold.provenance, RowProvenance::FormatDefault);
    assert_eq!(bold.value_display, None);
}

#[test]
fn local_character_property_is_marked_local() {
    let mut cat = StyleCatalog::new();
    let cp = CharProps {
        italic: Some(true),
        ..Default::default()
    };
    insert(&mut cat, char_style("Emph", None, cp));
    let rows = character_inspector_rows(&cat, &StyleId::new("Emph"));
    let italic = row(&rows, StyleProperty::Italic);
    assert!(italic.provenance.is_local());
    assert_eq!(italic.value_display.as_deref(), Some("On"));
}

#[test]
fn inherited_character_property_names_its_ancestor() {
    let mut cat = StyleCatalog::new();
    let base = CharProps {
        bold: Some(true),
        font_size: Some(Points::new(11.0)),
        ..Default::default()
    };
    insert(&mut cat, named(char_style("Emph", None, base), "Emphasis"));
    insert(
        &mut cat,
        char_style("Strong", Some("Emph"), CharProps::default()),
    );
    let rows = character_inspector_rows(&cat, &StyleId::new("Strong"));

    let bold = row(&rows, StyleProperty::Bold);
    assert_eq!(
        bold.provenance,
        RowProvenance::Inherited {
            ancestor_id: StyleId::new("Emph"),
            ancestor_display: "Emphasis".to_string(),
        }
    );
    assert_eq!(bold.value_display.as_deref(), Some("On"));
    assert_eq!(
        row(&rows, StyleProperty::FontSize).value_display.as_deref(),
        Some("11 pt")
    );
}

#[test]
fn unknown_character_style_yields_no_rows() {
    let cat = StyleCatalog::new();
    assert!(character_inspector_rows(&cat, &StyleId::new("Ghost")).is_empty());
}
