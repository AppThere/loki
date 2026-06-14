// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Model invariants and Loro CRDT round-trip tests for `loki-sheet-model`.

use loki_sheet_model::{
    CellAlign, CellStyle, DocumentMeta, NumberFormat, Workbook, Worksheet, loro_to_workbook,
    workbook_to_loro,
};

// ── Model invariants ───────────────────────────────────────────────────────────

#[test]
fn new_workbook_has_one_default_sheet() {
    let wb = Workbook::new();
    assert_eq!(wb.sheets.len(), 1);
    assert_eq!(wb.get_sheet(0).unwrap().name, "Sheet1");
    assert!(wb.get_sheet(1).is_none());
}

#[test]
fn get_cell_mut_creates_default_cell() {
    let mut sheet = Worksheet::new("S");
    assert!(sheet.get_cell(2, 3).is_none());
    sheet.get_cell_mut(2, 3).value = "42".to_string();
    assert_eq!(sheet.get_cell(2, 3).unwrap().value, "42");
}

#[test]
fn align_and_format_string_reprs_are_stable() {
    // These strings are the on-CRDT encoding; changing them silently breaks
    // round-tripping, so pin them.
    assert_eq!(CellAlign::Left.as_str(), "left");
    assert_eq!(CellAlign::Center.as_str(), "center");
    assert_eq!(CellAlign::Right.as_str(), "right");
    assert_eq!(NumberFormat::General.as_str(), "general");
    assert_eq!(NumberFormat::Currency.as_str(), "currency");
    assert_eq!(NumberFormat::Percent.as_str(), "percent");
}

// ── Round-trip helpers ──────────────────────────────────────────────────────────

fn round_trip(wb: &Workbook) -> Workbook {
    let doc = workbook_to_loro(wb).expect("workbook_to_loro");
    loro_to_workbook(&doc).expect("loro_to_workbook")
}

// ── Round-trip tests ────────────────────────────────────────────────────────────

#[test]
fn round_trip_metadata() {
    let mut wb = Workbook::new();
    wb.meta = DocumentMeta {
        title: Some("Quarterly".to_string()),
        creator: Some("Ada".to_string()),
    };
    let back = round_trip(&wb);
    assert_eq!(back.meta.title.as_deref(), Some("Quarterly"));
    assert_eq!(back.meta.creator.as_deref(), Some("Ada"));
}

#[test]
fn round_trip_absent_metadata_is_none() {
    let wb = Workbook::new();
    let back = round_trip(&wb);
    assert_eq!(back.meta, DocumentMeta::default());
}

#[test]
fn round_trip_plain_cell_value() {
    let mut wb = Workbook::new();
    wb.get_sheet_mut(0).unwrap().get_cell_mut(0, 0).value = "hello".to_string();
    let back = round_trip(&wb);
    let cell = back.get_sheet(0).unwrap().get_cell(0, 0).unwrap();
    assert_eq!(cell.value, "hello");
    assert_eq!(cell.formula, None);
    assert_eq!(cell.style, None);
}

#[test]
fn round_trip_formula_is_preserved() {
    let mut wb = Workbook::new();
    {
        let c = wb.get_sheet_mut(0).unwrap().get_cell_mut(5, 0);
        c.value = "15".to_string();
        c.formula = Some("=SUM(A1:A5)".to_string());
    }
    let back = round_trip(&wb);
    let cell = back.get_sheet(0).unwrap().get_cell(5, 0).unwrap();
    assert_eq!(cell.value, "15");
    assert_eq!(cell.formula.as_deref(), Some("=SUM(A1:A5)"));
}

#[test]
fn round_trip_full_cell_style() {
    let mut wb = Workbook::new();
    let style = CellStyle {
        bold: true,
        italic: false,
        underline: true,
        align: CellAlign::Right,
        num_format: NumberFormat::Currency,
    };
    wb.get_sheet_mut(0).unwrap().get_cell_mut(1, 1).style = Some(style.clone());
    let back = round_trip(&wb);
    let cell = back.get_sheet(0).unwrap().get_cell(1, 1).unwrap();
    assert_eq!(cell.style.as_ref(), Some(&style));
}

#[test]
fn round_trip_every_align_and_format_variant() {
    let aligns = [CellAlign::Left, CellAlign::Center, CellAlign::Right];
    let formats = [
        NumberFormat::General,
        NumberFormat::Currency,
        NumberFormat::Percent,
    ];
    let mut wb = Workbook::new();
    {
        let sheet = wb.get_sheet_mut(0).unwrap();
        for (r, align) in aligns.iter().enumerate() {
            for (c, num_format) in formats.iter().enumerate() {
                sheet.get_cell_mut(r as u32, c as u32).style = Some(CellStyle {
                    align: *align,
                    num_format: *num_format,
                    ..Default::default()
                });
            }
        }
    }
    let back = round_trip(&wb);
    let sheet = back.get_sheet(0).unwrap();
    for (r, align) in aligns.iter().enumerate() {
        for (c, num_format) in formats.iter().enumerate() {
            let style = sheet
                .get_cell(r as u32, c as u32)
                .unwrap()
                .style
                .as_ref()
                .unwrap();
            assert_eq!(style.align, *align);
            assert_eq!(style.num_format, *num_format);
        }
    }
}

#[test]
fn round_trip_multiple_sheets_and_names() {
    let mut wb = Workbook::new();
    wb.get_sheet_mut(0).unwrap().name = "First".to_string();
    let mut second = Worksheet::new("Second");
    second.get_cell_mut(0, 0).value = "x".to_string();
    wb.sheets.push(second);

    let back = round_trip(&wb);
    assert_eq!(back.sheets.len(), 2);
    assert_eq!(back.get_sheet(0).unwrap().name, "First");
    assert_eq!(back.get_sheet(1).unwrap().name, "Second");
    assert_eq!(
        back.get_sheet(1).unwrap().get_cell(0, 0).unwrap().value,
        "x"
    );
}

#[test]
fn empty_document_restores_a_default_sheet() {
    // A LoroDoc with no sheets list must still yield a usable workbook.
    let wb = Workbook {
        meta: DocumentMeta::default(),
        sheets: Vec::new(),
    };
    let back = round_trip(&wb);
    assert_eq!(back.sheets.len(), 1);
    assert_eq!(back.get_sheet(0).unwrap().name, "Sheet1");
}

#[test]
fn round_trip_preserves_high_cell_coordinates() {
    let mut wb = Workbook::new();
    wb.get_sheet_mut(0)
        .unwrap()
        .get_cell_mut(1_048_575, 16_383)
        .value = "corner".to_string();
    let back = round_trip(&wb);
    assert_eq!(
        back.get_sheet(0)
            .unwrap()
            .get_cell(1_048_575, 16_383)
            .unwrap()
            .value,
        "corner"
    );
}

#[test]
fn round_trip_column_widths() {
    let mut wb = Workbook::new();
    {
        let sheet = wb.get_sheet_mut(0).unwrap();
        sheet.set_column_width(0, 120.5);
        sheet.set_column_width(3, 48.0);
    }
    let back = round_trip(&wb);
    let sheet = back.get_sheet(0).unwrap();
    assert_eq!(sheet.column_width(0), Some(120.5));
    assert_eq!(sheet.column_width(3), Some(48.0));
    assert_eq!(sheet.column_width(1), None);
}

#[test]
fn round_trip_is_idempotent() {
    let mut wb = Workbook::new();
    wb.meta.title = Some("T".to_string());
    {
        let c = wb.get_sheet_mut(0).unwrap().get_cell_mut(3, 4);
        c.value = "v".to_string();
        c.formula = Some("=1+2".to_string());
        c.style = Some(CellStyle {
            bold: true,
            ..Default::default()
        });
    }
    let once = round_trip(&wb);
    let twice = round_trip(&once);
    assert_eq!(once, twice);
}
