// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

#![cfg(feature = "xlsx")]

use loki_ooxml::xlsx::export::XlsxExport;
use loki_ooxml::xlsx::import::{XlsxImport, XlsxImportOptions};
use loki_sheet_model::{Cell, CellAlign, CellStyle, NumberFormat, Workbook, Worksheet};
use std::io::Cursor;

#[test]
fn test_xlsx_round_trip_basic() {
    // 1. Create a structured workbook with various types of data and styles
    let mut workbook = Workbook::new();

    // Customize first sheet
    let sheet1 = workbook.get_sheet_mut(0).unwrap();
    sheet1.name = "Data Sheet".to_string();

    // (row, col) coordinates:
    // A1 (0, 0): string value
    sheet1.cells.insert(
        (0, 0),
        Cell {
            value: "Product Name".to_string(),
            formula: None,
            style: Some(CellStyle {
                bold: true,
                italic: false,
                underline: true,
                align: CellAlign::Left,
                num_format: NumberFormat::General,
            }),
        },
    );

    // B1 (0, 1): string value, right aligned
    sheet1.cells.insert(
        (0, 1),
        Cell {
            value: "Unit Cost".to_string(),
            formula: None,
            style: Some(CellStyle {
                bold: true,
                italic: true,
                underline: false,
                align: CellAlign::Right,
                num_format: NumberFormat::General,
            }),
        },
    );

    // C1 (0, 2): string value, center aligned
    sheet1.cells.insert(
        (0, 2),
        Cell {
            value: "Quantity".to_string(),
            formula: None,
            style: Some(CellStyle {
                bold: true,
                italic: false,
                underline: false,
                align: CellAlign::Center,
                num_format: NumberFormat::General,
            }),
        },
    );

    // D1 (0, 3): string value
    sheet1.cells.insert(
        (0, 3),
        Cell {
            value: "Total Margin %".to_string(),
            formula: None,
            style: Some(CellStyle {
                bold: true,
                italic: true,
                underline: true,
                align: CellAlign::Center,
                num_format: NumberFormat::General,
            }),
        },
    );

    // A2 (1, 0): string value
    sheet1.cells.insert(
        (1, 0),
        Cell {
            value: "Widget A".to_string(),
            formula: None,
            style: None,
        },
    );

    // B2 (1, 1): numeric value
    sheet1.cells.insert(
        (1, 1),
        Cell {
            value: "123.45".to_string(),
            formula: None,
            style: Some(CellStyle {
                bold: false,
                italic: false,
                underline: false,
                align: CellAlign::Right,
                num_format: NumberFormat::Currency,
            }),
        },
    );

    // C2 (1, 2): numeric value
    sheet1.cells.insert(
        (1, 2),
        Cell {
            value: "10".to_string(),
            formula: None,
            style: Some(CellStyle {
                bold: false,
                italic: false,
                underline: false,
                align: CellAlign::Right,
                num_format: NumberFormat::General,
            }),
        },
    );

    // D2 (1, 3): percentage value
    sheet1.cells.insert(
        (1, 3),
        Cell {
            value: "0.25".to_string(),
            formula: None,
            style: Some(CellStyle {
                bold: false,
                italic: false,
                underline: false,
                align: CellAlign::Right,
                num_format: NumberFormat::Percent,
            }),
        },
    );

    // A3 (2, 0): string value
    sheet1.cells.insert(
        (2, 0),
        Cell {
            value: "Total Cost".to_string(),
            formula: None,
            style: Some(CellStyle {
                bold: true,
                italic: false,
                underline: false,
                align: CellAlign::Left,
                num_format: NumberFormat::General,
            }),
        },
    );

    // B3 (2, 1): formula value (note: formula without starting = because parser expects formula without prefix)
    sheet1.cells.insert(
        (2, 1),
        Cell {
            value: "1234.50".to_string(), // evaluated value
            formula: Some("SUM(B2*C2)".to_string()),
            style: Some(CellStyle {
                bold: true,
                italic: false,
                underline: false,
                align: CellAlign::Right,
                num_format: NumberFormat::Currency,
            }),
        },
    );

    // Let's add a second sheet to check multi-sheet capabilities
    let mut sheet2 = Worksheet::new("Summary Sheet");
    sheet2.cells.insert(
        (0, 0),
        Cell {
            value: "Overall Summary".to_string(),
            formula: None,
            style: Some(CellStyle {
                bold: true,
                italic: true,
                underline: false,
                align: CellAlign::Left,
                num_format: NumberFormat::General,
            }),
        },
    );
    workbook.sheets.push(sheet2);

    // 2. Export the workbook into a byte buffer
    let mut buffer = Cursor::new(Vec::new());
    XlsxExport::export(&workbook, &mut buffer).expect("Export should succeed");

    // 3. Re-import the workbook from the byte buffer
    buffer.set_position(0);
    let imported_workbook = XlsxImport::import(&mut buffer, XlsxImportOptions::default())
        .expect("Import should succeed");

    // 4. Assert correctness
    assert_eq!(imported_workbook.sheets.len(), workbook.sheets.len());

    for (sheet_idx, original_sheet) in workbook.sheets.iter().enumerate() {
        let imported_sheet = &imported_workbook.sheets[sheet_idx];
        assert_eq!(imported_sheet.name, original_sheet.name);

        // Let's verify each cell
        for (coord, original_cell) in &original_sheet.cells {
            let imported_cell = imported_sheet.cells.get(coord).expect(&format!(
                "Cell at {:?} should exist in imported sheet",
                coord
            ));

            assert_eq!(
                imported_cell.value, original_cell.value,
                "Cell value mismatch at sheet {}, coord {:?}",
                sheet_idx, coord
            );
            assert_eq!(
                imported_cell.formula, original_cell.formula,
                "Cell formula mismatch at sheet {}, coord {:?}",
                sheet_idx, coord
            );

            // Check styles
            match (&original_cell.style, &imported_cell.style) {
                (None, None) => {}
                (Some(orig_style), Some(imp_style)) => {
                    assert_eq!(
                        imp_style.bold, orig_style.bold,
                        "Bold style mismatch at {:?}",
                        coord
                    );
                    assert_eq!(
                        imp_style.italic, orig_style.italic,
                        "Italic style mismatch at {:?}",
                        coord
                    );
                    assert_eq!(
                        imp_style.underline, orig_style.underline,
                        "Underline style mismatch at {:?}",
                        coord
                    );
                    assert_eq!(
                        imp_style.align, orig_style.align,
                        "Alignment mismatch at {:?}",
                        coord
                    );
                    assert_eq!(
                        imp_style.num_format, orig_style.num_format,
                        "Number format mismatch at {:?}",
                        coord
                    );
                }
                (Some(orig_style), None) => {
                    // Check if the original style was actually just the default one.
                    // If it was default style, it might not be exported/imported as Some(style).
                    let default_style = CellStyle::default();
                    assert_eq!(
                        orig_style, &default_style,
                        "Non-default style was lost at {:?}",
                        coord
                    );
                }
                (None, Some(imp_style)) => {
                    // Check if imported style matches the default style.
                    let default_style = CellStyle::default();
                    assert_eq!(
                        imp_style, &default_style,
                        "Extraneous non-default style imported at {:?}",
                        coord
                    );
                }
            }
        }
    }
}
