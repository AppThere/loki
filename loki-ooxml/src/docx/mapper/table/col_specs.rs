// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Column-spec and table-width helpers.

use loki_doc_model::content::table::col::{ColAlignment, ColSpec, ColWidth, TableWidth};
use loki_primitives::units::Points;

use crate::docx::model::styles::DocxTableModel;

/// Converts `w:tblW` to [`TableWidth`].
///
/// COMPAT(microsoft): w:tblW @w:type="pct" uses fiftieths of a percent,
/// not hundredths — divide by 50 to get 0.0–100.0 range.
#[allow(clippy::cast_precision_loss)] // twip values are small; f32 precision is sufficient
pub(crate) fn map_tbl_width(t: &DocxTableModel) -> Option<TableWidth> {
    let w = t.tbl_pr.as_ref()?.width.as_ref()?;
    Some(match w.w_type.as_str() {
        "dxa" => TableWidth::Fixed(w.w as f32 / 20.0),
        "pct" => TableWidth::Percent(w.w as f32 / 50.0),
        _ => TableWidth::Auto, // "auto" | "nil" | unknown
    })
}

/// Builds column specifications from `w:tblGrid` column widths.
pub(crate) fn build_col_specs(t: &DocxTableModel) -> Vec<ColSpec> {
    if t.col_widths.is_empty() {
        // Fall back: infer column count from the widest row.
        let num_cols = t.rows.iter().map(|r| r.cells.len()).max().unwrap_or(0);
        (0..num_cols)
            .map(|_| ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Default,
            })
            .collect()
    } else {
        t.col_widths
            .iter()
            .map(|&w| ColSpec {
                alignment: ColAlignment::Default,
                width: if w > 0 {
                    ColWidth::Fixed(Points::new(f64::from(w) / 20.0))
                } else {
                    ColWidth::Default
                },
            })
            .collect()
    }
}
