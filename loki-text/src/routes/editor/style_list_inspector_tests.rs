// SPDX-License-Identifier: Apache-2.0

//! List-style inspector rows (Spec 05 M6 list family): one flattened row per
//! indent level, non-inheriting.

use super::*;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::{ListStyle, StyleCatalog};

fn level(n: u8, kind: ListLevelKind, indent: f64, hanging: f64) -> ListLevel {
    ListLevel {
        level: n,
        kind,
        indent_start: Points::new(indent),
        hanging_indent: Points::new(hanging),
        label_alignment: LabelAlignment::Left,
        tab_stop_after_label: None,
        char_props: CharProps::default(),
    }
}

fn insert(cat: &mut StyleCatalog, id: &str, levels: Vec<ListLevel>) {
    cat.list_styles.insert(
        ListId::new(id),
        ListStyle {
            id: ListId::new(id),
            display_name: None,
            levels,
            extensions: Default::default(),
        },
    );
}

#[test]
fn one_row_per_level_in_order() {
    let mut cat = StyleCatalog::new();
    insert(
        &mut cat,
        "L",
        vec![
            level(
                0,
                ListLevelKind::Bullet {
                    char: BulletChar::Char('•'),
                    font: None,
                },
                18.0,
                18.0,
            ),
            level(
                1,
                ListLevelKind::Bullet {
                    char: BulletChar::Char('◦'),
                    font: None,
                },
                36.0,
                18.0,
            ),
        ],
    );
    let rows = list_inspector_rows(&cat, &ListId::new("L"));
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].level, 0);
    assert_eq!(rows[1].level, 1);
}

#[test]
fn bullet_level_shows_its_char_and_geometry() {
    let mut cat = StyleCatalog::new();
    insert(
        &mut cat,
        "L",
        vec![level(
            0,
            ListLevelKind::Bullet {
                char: BulletChar::Char('•'),
                font: None,
            },
            18.0,
            9.0,
        )],
    );
    let rows = list_inspector_rows(&cat, &ListId::new("L"));
    assert_eq!(rows[0].label, "Bullet •");
    assert_eq!(rows[0].indent, "18 pt");
    assert_eq!(rows[0].hanging, "9 pt");
    assert_eq!(rows[0].alignment, "Left");
}

#[test]
fn numbered_level_names_its_scheme() {
    let mut cat = StyleCatalog::new();
    insert(
        &mut cat,
        "L",
        vec![level(
            0,
            ListLevelKind::Numbered {
                scheme: NumberingScheme::Decimal,
                start_value: 1,
                format: "%1.".to_string(),
                display_levels: 1,
            },
            18.0,
            18.0,
        )],
    );
    let rows = list_inspector_rows(&cat, &ListId::new("L"));
    assert_eq!(rows[0].label, "Numbered · Decimal");
}

#[test]
fn unknown_list_style_yields_no_rows() {
    let cat = StyleCatalog::new();
    assert!(list_inspector_rows(&cat, &ListId::new("Ghost")).is_empty());
}
