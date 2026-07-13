// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Spec 02 round-trip axis — per-case **P0 fidelity** assertions for XLSX
//! formulas (audit T-3).
//!
//! Like `conformance_p0_round_trip.rs` (DOCX), each test builds a workbook
//! carrying a specific formula, runs a single `export → re-import`, and asserts
//! the formula *body* survived — the stronger guard that catches first-export
//! loss, which the divergence-based `conformance_xlsx_round_trip.rs` cannot.
//!
//! The exporter stores `<f>` without a leading `=` and the importer reads it
//! back verbatim, so the round-trip normalises away a leading `=`; the
//! assertions compare the `=`-stripped body, which is the semantically
//! meaningful part.
//!
//! - **TC-XLSX-009** — dynamic-array spill formulas (`SEQUENCE`, ranges,
//!   `SORT`, `UNIQUE`).
//! - **TC-XLSX-011** — modern functions (`XLOOKUP`, `LET`, `FILTER`,
//!   `TEXTJOIN`).

#![cfg(feature = "xlsx")]

use std::io::Cursor;

use loki_ooxml::xlsx::export::XlsxExport;
use loki_ooxml::xlsx::import::{XlsxImport, XlsxImportOptions};
use loki_sheet_model::{Workbook, Worksheet};

fn export(wb: &Workbook) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    XlsxExport::export(wb, &mut buf).expect("XLSX export should succeed");
    buf.into_inner()
}

fn import(bytes: Vec<u8>) -> Workbook {
    XlsxImport::import(Cursor::new(bytes), XlsxImportOptions::default())
        .expect("XLSX should import")
}

/// Normalise a formula for comparison: drop a single leading `=`.
fn norm(f: &str) -> &str {
    f.strip_prefix('=').unwrap_or(f)
}

/// Round-trips a single-cell workbook whose A1 carries `formula`, and returns
/// the re-imported formula string (or `None` if the cell/formula was lost).
fn round_trip_formula(formula: &str) -> Option<String> {
    let mut sheet = Worksheet::new("Formulas");
    let cell = sheet.get_cell_mut(0, 0);
    // A cached result keeps the cell non-empty (spill anchors carry one in
    // practice); the formula is the property under test.
    cell.value = "0".to_string();
    cell.formula = Some(formula.to_string());

    let mut wb = Workbook::new();
    wb.sheets = vec![sheet];

    let re = import(export(&wb));
    re.sheets
        .first()
        .and_then(|s| s.cells.get(&(0, 0)))
        .and_then(|c| c.formula.clone())
}

fn assert_formula_round_trips(formula: &str) {
    let got = round_trip_formula(formula)
        .unwrap_or_else(|| panic!("formula `{formula}` was lost in XLSX round-trip"));
    assert_eq!(
        norm(&got),
        norm(formula),
        "formula body must survive XLSX round-trip"
    );
}

/// TC-XLSX-009 — dynamic-array / spill formulas must survive an
/// export→re-import with their body intact.
#[test]
fn tc_xlsx_009_dynamic_array_spill_formulas_round_trip() {
    for f in [
        "=SEQUENCE(3,1)",
        "=A1:A3*2",
        "=SORT(A1:A10)",
        "=UNIQUE(A1:A20)",
    ] {
        assert_formula_round_trips(f);
    }
}

/// TC-XLSX-011 — modern functions (XLOOKUP and friends) must survive an
/// export→re-import with their body intact.
#[test]
fn tc_xlsx_011_modern_function_formulas_round_trip() {
    for f in [
        "=XLOOKUP(E1,A1:A10,B1:B10)",
        "=LET(x,A1,x*2)",
        "=FILTER(A1:A10,B1:B10>0)",
        "=TEXTJOIN(\",\",TRUE,A1:A5)",
    ] {
        assert_formula_round_trips(f);
    }
}
