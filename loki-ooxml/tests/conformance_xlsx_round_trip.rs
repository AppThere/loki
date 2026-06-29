// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Spec 02 round-trip axis — XLSX **import-export-import** stability.
//!
//! The spreadsheet analogue of `conformance_round_trip.rs`: both compared
//! workbooks are *imported*, so any divergence is a genuine export→re-import
//! loss, reported with a `sheet0000/r0001c0002/…` model path by
//! `appthere_conformance::sheet` rather than a bespoke per-cell assertion.

#![cfg(feature = "xlsx")]

use std::io::Cursor;

use appthere_conformance::roundtrip::{Divergence, first_divergence};
use appthere_conformance::sheet::canonicalize_workbook;
use loki_ooxml::xlsx::export::XlsxExport;
use loki_ooxml::xlsx::import::{XlsxImport, XlsxImportOptions};
use loki_sheet_model::{CellStyle, NumberFormat, Workbook, Worksheet};

fn import(bytes: Vec<u8>) -> Workbook {
    XlsxImport::import(Cursor::new(bytes), XlsxImportOptions::default())
        .expect("XLSX should import")
}

fn export(wb: &Workbook) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    XlsxExport::export(wb, &mut buf).expect("XLSX export should succeed");
    buf.into_inner()
}

/// First divergence of `seed` under XLSX import-export-import.
fn round_trip_divergence(seed: &Workbook) -> Option<Divergence> {
    let a = import(export(seed));
    let b = import(export(&a));
    first_divergence(&canonicalize_workbook(&a), &canonicalize_workbook(&b))
}

/// A workbook with values, a formula, a styled cell, and a custom column width
/// must survive an XLSX export→re-import with no model divergence.
#[test]
fn xlsx_round_trip_preserves_core_content() {
    let mut sheet = Worksheet::new("Data");
    sheet.get_cell_mut(0, 0).value = "Item".to_string();
    sheet.get_cell_mut(0, 1).value = "Qty".to_string();
    sheet.get_cell_mut(1, 0).value = "Widgets".to_string();
    sheet.get_cell_mut(1, 1).value = "42".to_string();
    let total = sheet.get_cell_mut(2, 1);
    total.value = "42".to_string();
    total.formula = Some("=SUM(B2:B2)".to_string());

    // A bold, percent-formatted header cell.
    sheet.get_cell_mut(0, 0).style = Some(CellStyle {
        bold: true,
        num_format: NumberFormat::Percent,
        ..Default::default()
    });
    sheet.set_column_width(0, 120.0);

    let mut seed = Workbook::new();
    seed.sheets = vec![sheet];

    if let Some(d) = round_trip_divergence(&seed) {
        panic!(
            "core XLSX round-trip diverged at `{}`:\n  first import: {:?}\n  re-import:    {:?}",
            d.path, d.left, d.right
        );
    }
}
