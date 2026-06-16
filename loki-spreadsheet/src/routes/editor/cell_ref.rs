// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! A1-style cell-reference parsing and column-label generation.
//!
//! Replaces the original fixed `A`..`J` / row-`30` scheme with arbitrary
//! multi-letter columns and full-height rows, bounded by the spreadsheet
//! formats' hard limits so malformed references can't produce runaway indices.

/// Maximum addressable column index (0-based). XLSX caps columns at 16,384
/// (`A`..`XFD`), so the last valid index is 16,383.
pub const MAX_COL: usize = 16_383;

/// Maximum addressable row index (0-based). XLSX caps rows at 1,048,576, so the
/// last valid index is 1,048,575.
pub const MAX_ROW: usize = 1_048_575;

/// Parses an A1-style reference (e.g. `"B2"`, `"AA10"`) into 0-based
/// `(row, col)` coordinates.
///
/// Case-insensitive. Returns `None` for malformed references or for
/// coordinates beyond [`MAX_ROW`] / [`MAX_COL`].
pub fn parse_cell_ref(s: &str) -> Option<(usize, usize)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // Split into the leading letter run (column) and trailing digit run (row).
    let split = s.find(|c: char| c.is_ascii_digit())?;
    if split == 0 {
        return None; // no column letters
    }
    let (col_part, row_part) = s.split_at(split);

    let col = col_from_label(col_part)?;
    if col > MAX_COL {
        return None;
    }

    // Row part must be all digits.
    if !row_part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let row_1based = row_part.parse::<usize>().ok()?;
    let row = row_1based.checked_sub(1)?;
    if row > MAX_ROW {
        return None;
    }

    Some((row, col))
}

/// Converts a 0-based column index into its A1 label (`0` → `"A"`, `25` → `"Z"`,
/// `26` → `"AA"`).
pub fn col_to_label(mut col: usize) -> String {
    let mut label = String::new();
    loop {
        let rem = col % 26;
        label.insert(0, (b'A' + rem as u8) as char);
        if col < 26 {
            break;
        }
        col = col / 26 - 1;
    }
    label
}

/// The visible grid extent `(rows, cols)` for a workbook: large enough to show
/// every populated cell of the first sheet plus a little padding, clamped to a
/// render-friendly maximum and a comfortable minimum.
///
/// Cells beyond the cap are still preserved on save — this only bounds how much
/// is rendered. Full virtualization (showing arbitrarily large sheets without a
/// cap) is a follow-up.
pub fn grid_dimensions(wb: &loki_sheet_model::Workbook) -> (usize, usize) {
    const MIN_ROWS: usize = 30;
    const MIN_COLS: usize = 12;
    const PAD_ROWS: usize = 10;
    const PAD_COLS: usize = 3;
    const CAP_ROWS: usize = 500;
    const CAP_COLS: usize = 52;

    let (mut max_r, mut max_c) = (0usize, 0usize);
    if let Some(sheet) = wb.get_sheet(0) {
        for &(r, c) in sheet.cells.keys() {
            max_r = max_r.max(r as usize);
            max_c = max_c.max(c as usize);
        }
    }
    let rows = (max_r + 1 + PAD_ROWS).clamp(MIN_ROWS, CAP_ROWS);
    let cols = (max_c + 1 + PAD_COLS).clamp(MIN_COLS, CAP_COLS);
    (rows, cols)
}

/// Converts an A1 column label (e.g. `"A"`, `"aa"`) into a 0-based index.
///
/// Returns `None` if the label is empty or contains a non-letter.
pub fn col_from_label(label: &str) -> Option<usize> {
    if label.is_empty() {
        return None;
    }
    let mut col: usize = 0;
    for ch in label.chars() {
        let upper = ch.to_ascii_uppercase();
        if !upper.is_ascii_alphabetic() {
            return None;
        }
        let digit = (upper as u8 - b'A') as usize + 1; // bijective base-26
        col = col.checked_mul(26)?.checked_add(digit)?;
    }
    Some(col - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_refs() {
        assert_eq!(parse_cell_ref("A1"), Some((0, 0)));
        assert_eq!(parse_cell_ref("B2"), Some((1, 1)));
        assert_eq!(parse_cell_ref("J30"), Some((29, 9)));
    }

    #[test]
    fn parses_multi_letter_columns() {
        assert_eq!(parse_cell_ref("Z1"), Some((0, 25)));
        assert_eq!(parse_cell_ref("AA1"), Some((0, 26)));
        assert_eq!(parse_cell_ref("AB10"), Some((9, 27)));
        assert_eq!(parse_cell_ref("XFD1"), Some((0, MAX_COL)));
    }

    #[test]
    fn is_case_insensitive_and_trims() {
        assert_eq!(parse_cell_ref("  b2 "), Some((1, 1)));
        assert_eq!(parse_cell_ref("aa1"), Some((0, 26)));
    }

    #[test]
    fn rejects_malformed_refs() {
        assert_eq!(parse_cell_ref(""), None);
        assert_eq!(parse_cell_ref("1"), None); // no column
        assert_eq!(parse_cell_ref("A"), None); // no row
        assert_eq!(parse_cell_ref("A0"), None); // rows are 1-based
        assert_eq!(parse_cell_ref("A1B"), None); // trailing letters
        assert_eq!(parse_cell_ref("A1.5"), None);
        assert_eq!(parse_cell_ref("#REF!"), None);
    }

    #[test]
    fn rejects_out_of_range() {
        assert_eq!(parse_cell_ref("XFE1"), None); // one past max column
        assert_eq!(parse_cell_ref("A1048577"), None); // one past max row
    }

    #[test]
    fn column_labels_round_trip() {
        for col in [0usize, 1, 25, 26, 27, 51, 52, 701, 702, MAX_COL] {
            let label = col_to_label(col);
            assert_eq!(col_from_label(&label), Some(col), "label {label}");
        }
        assert_eq!(col_to_label(0), "A");
        assert_eq!(col_to_label(25), "Z");
        assert_eq!(col_to_label(26), "AA");
        assert_eq!(col_to_label(701), "ZZ");
        assert_eq!(col_to_label(702), "AAA");
    }

    #[test]
    fn col_from_label_rejects_non_letters() {
        assert_eq!(col_from_label(""), None);
        assert_eq!(col_from_label("A1"), None);
        assert_eq!(col_from_label("?"), None);
    }

    #[test]
    fn grid_dimensions_default_minimum() {
        let wb = loki_sheet_model::Workbook::new();
        assert_eq!(grid_dimensions(&wb), (30, 12));
    }

    #[test]
    fn grid_dimensions_expand_to_used_range_with_padding() {
        let mut wb = loki_sheet_model::Workbook::new();
        wb.get_sheet_mut(0).unwrap().get_cell_mut(40, 20).value = "x".to_string();
        // 40 + 1 + 10 = 51 rows; 20 + 1 + 3 = 24 cols.
        assert_eq!(grid_dimensions(&wb), (51, 24));
    }

    #[test]
    fn grid_dimensions_clamped_to_cap() {
        let mut wb = loki_sheet_model::Workbook::new();
        wb.get_sheet_mut(0)
            .unwrap()
            .get_cell_mut(900_000, 5_000)
            .value = "x".to_string();
        assert_eq!(grid_dimensions(&wb), (500, 52));
    }
}
