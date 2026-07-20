// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the named table-style model (extracted from `table_style.rs`
//! to keep it under the file-size ceiling). The `TableBorders` edge-resolution
//! test lives with its type in `table_borders.rs`.

use super::*;

#[test]
fn table_style_default_props() {
    let style = TableStyle {
        id: StyleId("TableGrid".into()),
        display_name: Some("Table Grid".into()),
        parent: None,
        table_props: TableProps::default(),
        conditional: IndexMap::new(),
        extensions: ExtensionBag::default(),
    };
    assert!(style.table_props.width.is_none());
    assert!(style.table_props.border.is_none());
    assert!(style.table_props.borders.is_none());
    assert!(style.conditional.is_empty());
}

#[test]
fn default_table_look_matches_word_04a0() {
    let look = TableLook::default();
    assert!(look.first_row);
    assert!(look.first_column);
    assert!(look.horizontal_banding);
    assert!(!look.last_row);
    assert!(!look.last_column);
    assert!(!look.vertical_banding);
}

#[test]
fn table_look_attr_round_trips() {
    for look in [
        TableLook::default(),
        TableLook {
            first_row: false,
            last_row: true,
            first_column: false,
            last_column: true,
            horizontal_banding: false,
            vertical_banding: true,
        },
    ] {
        assert_eq!(TableLook::decode_attr(&look.encode_attr()), Some(look));
    }
    assert_eq!(TableLook::default().encode_attr(), "101010");
}

#[test]
fn table_look_decode_rejects_malformed() {
    assert_eq!(TableLook::decode_attr(""), None);
    assert_eq!(TableLook::decode_attr("10101"), None);
    assert_eq!(TableLook::decode_attr("1010102"), None);
    assert_eq!(TableLook::decode_attr("10101x"), None);
}
