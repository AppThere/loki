// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Round-trip canonicalization for the spreadsheet model
//! (`loki_sheet_model::Workbook`), feature `sheet-model`.
//!
//! Implements [`NormalizedModel`](crate::roundtrip::NormalizedModel) for
//! `Workbook`, so the round-trip axis works on spreadsheets the same way the
//! [`model`](crate::model) adapter does for word-processing documents. The
//! canonical form is an order-stable sequence of `(path, value)` entries
//! capturing the semantically significant content — workbook metadata, each
//! sheet's name, every populated cell's value / formula / style, and custom
//! column widths. Cells and widths live in `HashMap`s, so the keys are **sorted**
//! before emission to keep the order deterministic;
//! [`crate::roundtrip::first_divergence`] then pinpoints the first loss with a
//! `sheet0000/r0001c0002/…` path.

use loki_sheet_model::workbook::{Cell, CellStyle, Workbook};

use crate::roundtrip::{CanonicalEntry, NormalizedModel};

impl NormalizedModel for Workbook {
    fn canonical(&self) -> Vec<CanonicalEntry> {
        canonicalize_workbook(self)
    }
}

/// Produces the canonical `(path, value)` form of `wb` (see the module docs).
#[must_use]
pub fn canonicalize_workbook(wb: &Workbook) -> Vec<CanonicalEntry> {
    let mut out = Vec::new();
    if let Some(t) = &wb.meta.title {
        push(&mut out, "meta/title", t.clone());
    }
    if let Some(c) = &wb.meta.creator {
        push(&mut out, "meta/creator", c.clone());
    }
    for (si, sheet) in wb.sheets.iter().enumerate() {
        let sp = format!("sheet{si:04}");
        push(&mut out, format!("{sp}/name"), sheet.name.clone());

        // Cells are keyed by (row, col) in a HashMap — sort for a stable order.
        let mut cells: Vec<_> = sheet.cells.iter().collect();
        cells.sort_by(|a, b| a.0.cmp(b.0));
        for (&(r, c), cell) in cells {
            walk_cell(cell, &format!("{sp}/r{r:04}c{c:04}"), &mut out);
        }

        // Custom column widths, likewise sorted by column index.
        let mut widths: Vec<_> = sheet.column_widths.iter().collect();
        widths.sort_by(|a, b| a.0.cmp(b.0));
        for (&col, w) in widths {
            push(&mut out, format!("{sp}/col{col:04}/width"), format!("{w}"));
        }
    }
    out
}

fn walk_cell(cell: &Cell, path: &str, out: &mut Vec<CanonicalEntry>) {
    push(out, format!("{path}/value"), cell.value.clone());
    if let Some(f) = &cell.formula {
        push(out, format!("{path}/formula"), f.clone());
    }
    if let Some(s) = &cell.style {
        push(out, format!("{path}/style"), style_summary(s));
    }
}

/// A compact, stable serialization of a cell's style — only the set flags, in a
/// fixed order, so dropping any one changes the string and the differ catches it.
fn style_summary(s: &CellStyle) -> String {
    let mut parts: Vec<String> = Vec::new();
    if s.bold {
        parts.push("bold".to_string());
    }
    if s.italic {
        parts.push("italic".to_string());
    }
    if s.underline {
        parts.push("underline".to_string());
    }
    parts.push(format!("align={}", s.align.as_str()));
    parts.push(format!("numfmt={}", s.num_format.as_str()));
    parts.join(";")
}

fn push(out: &mut Vec<CanonicalEntry>, path: impl Into<String>, value: impl Into<String>) {
    out.push(CanonicalEntry::new(path, value));
}

#[cfg(test)]
#[path = "sheet_tests.rs"]
mod tests;
