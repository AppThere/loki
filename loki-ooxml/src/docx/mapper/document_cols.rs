// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `w:cols` → [`SectionColumns`] mapping, including unequal-width columns
//! (`w:equalWidth="0"`, feature 5.10). Split out of `mapper/document.rs` to hold
//! that file's line ceiling.

use loki_doc_model::layout::page::SectionColumns;
use loki_primitives::units::Points;

use crate::docx::model::section::DocxCols;

/// Map a section's `w:cols` to [`SectionColumns`]. Returns `None` for an absent
/// or single-column definition (only two-plus columns are meaningful).
pub(super) fn map_columns(cols: Option<&DocxCols>) -> Option<SectionColumns> {
    let cols = cols?;
    if cols.num < 2 {
        return None;
    }
    let count = u8::try_from(cols.num.clamp(2, i32::from(u8::MAX))).unwrap_or(2);
    // Unequal columns (`w:equalWidth="0"`): one `w:col @w:w` per column, in
    // twips. Only honoured when a width is present for every column.
    let widths = if cols.col_widths.len() == usize::from(count) {
        cols.col_widths
            .iter()
            .map(|w| Points::new(f64::from(*w) / 20.0))
            .collect()
    } else {
        Vec::new()
    };
    Some(SectionColumns {
        count,
        gap: Points::new(f64::from(cols.space) / 20.0),
        separator: cols.sep,
        widths,
    })
}
