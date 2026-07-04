// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Canonicalization of table interiors — column count, caption, and every row
//! (head, per-body head + body, foot) with each cell's span and block content.
//!
//! Cell blocks recurse through the parent module's [`walk_block`](super::walk_block),
//! so a table cell containing a styled paragraph, a list, or a nested table is
//! canonicalised to the same depth as top-level content. A dropped row, a
//! merged-away cell, or lost cell text therefore surfaces as a first divergence
//! with a `…/tbl/br0001/c0000/…` path.

use loki_doc_model::content::table::core::Table;
use loki_doc_model::content::table::row::Row;

use super::{push, walk_block, walk_inlines};
use crate::roundtrip::CanonicalEntry;

/// Walks `tbl` into canonical entries under `path`.
pub(super) fn walk_table(tbl: &Table, path: &str, out: &mut Vec<CanonicalEntry>) {
    push(out, format!("{path}/cols"), tbl.col_specs.len().to_string());
    if !tbl.caption.full.is_empty() {
        walk_inlines(&tbl.caption.full, &format!("{path}/caption"), out);
    }
    // A single monotonic row index across head/body/foot keeps the path stable
    // even if rows move between groups (the `group` letter records the section).
    let mut row_idx = 0usize;
    for row in &tbl.head.rows {
        walk_row(row, path, 'h', &mut row_idx, out);
    }
    for body in &tbl.bodies {
        for row in body.head_rows.iter().chain(body.body_rows.iter()) {
            walk_row(row, path, 'b', &mut row_idx, out);
        }
    }
    for row in &tbl.foot.rows {
        walk_row(row, path, 'f', &mut row_idx, out);
    }
}

fn walk_row(
    row: &Row,
    path: &str,
    group: char,
    row_idx: &mut usize,
    out: &mut Vec<CanonicalEntry>,
) {
    let rp = format!("{path}/{group}r{:04}", *row_idx);
    *row_idx += 1;
    for (ci, cell) in row.cells.iter().enumerate() {
        let cp = format!("{rp}/c{ci:04}");
        if cell.row_span != 1 || cell.col_span != 1 {
            push(
                out,
                format!("{cp}/span"),
                format!("{}x{}", cell.row_span, cell.col_span),
            );
        }
        for (bi, b) in cell.blocks.iter().enumerate() {
            walk_block(b, &format!("{cp}/{bi:04}"), out);
        }
    }
}
