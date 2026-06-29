// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use loki_sheet_model::workbook::{CellStyle, Workbook, Worksheet};

use crate::roundtrip::diff_models;

/// A workbook with a single sheet whose `(row, col)` cells carry the given
/// `(value, formula)` pairs.
fn wb(cells: &[((u32, u32), &str, Option<&str>)]) -> Workbook {
    let mut sheet = Worksheet::new("Sheet1");
    for &((r, c), value, formula) in cells {
        let cell = sheet.get_cell_mut(r, c);
        cell.value = value.to_string();
        cell.formula = formula.map(str::to_string);
    }
    let mut w = Workbook::new();
    w.sheets = vec![sheet];
    w
}

#[test]
fn identical_workbooks_round_trip_clean() {
    let a = wb(&[((0, 0), "1", None), ((0, 1), "2", None)]);
    assert_eq!(diff_models(&a, &a.clone()), None);
}

#[test]
fn dropped_cell_value_is_caught_with_a_coord_path() {
    let a = wb(&[((0, 0), "hello", None), ((2, 3), "world", None)]);
    let b = wb(&[((0, 0), "hello", None), ((2, 3), "", None)]);

    let d = diff_models(&a, &b).expect("changed cell value must be caught");
    assert!(d.path.ends_with("/r0002c0003/value"), "path = {}", d.path);
    assert_eq!(d.left.as_deref(), Some("world"));
    assert_eq!(d.right.as_deref(), Some(""));
}

#[test]
fn dropped_formula_is_caught() {
    let a = wb(&[((0, 0), "3", Some("=1+2"))]);
    let b = wb(&[((0, 0), "3", None)]);

    let d = diff_models(&a, &b).expect("dropped formula must be caught");
    assert!(d.path.ends_with("/formula"), "path = {}", d.path);
    assert_eq!(d.left.as_deref(), Some("=1+2"));
    assert!(d.right.is_none(), "right should lack the formula entry");
}

#[test]
fn lost_cell_style_is_caught() {
    let mut a = wb(&[((0, 0), "x", None)]);
    a.sheets[0].get_cell_mut(0, 0).style = Some(CellStyle {
        bold: true,
        ..Default::default()
    });
    let b = wb(&[((0, 0), "x", None)]);

    let d = diff_models(&a, &b).expect("lost style must be caught");
    assert!(d.path.ends_with("/style"), "path = {}", d.path);
    assert!(d.left.as_deref().unwrap_or_default().contains("bold"));
    assert!(d.right.is_none());
}

#[test]
fn renamed_sheet_is_caught() {
    let a = wb(&[((0, 0), "x", None)]);
    let mut b = wb(&[((0, 0), "x", None)]);
    b.sheets[0].name = "Renamed".to_string();

    let d = diff_models(&a, &b).expect("renamed sheet must be caught");
    assert!(d.path.ends_with("/name"), "path = {}", d.path);
    assert_eq!(d.left.as_deref(), Some("Sheet1"));
    assert_eq!(d.right.as_deref(), Some("Renamed"));
}
