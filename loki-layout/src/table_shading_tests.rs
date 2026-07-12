// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the layout-side table-style shading bridge.

use super::*;
use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::style::table_style::{TableConditionalFormat, TableProps, TableRegion};
use loki_primitives::color::RgbColor;

fn rgb(r: u8, g: u8, b: u8) -> DocumentColor {
    DocumentColor::Rgb(RgbColor::new(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
    ))
}

fn styled(id: &str, region: TableRegion, color: DocumentColor) -> TableStyle {
    let mut style = TableStyle {
        id: StyleId::new(id),
        display_name: None,
        parent: None,
        table_props: TableProps::default(),
        conditional: Default::default(),
        extensions: ExtensionBag::default(),
    };
    style.conditional.insert(
        region,
        TableConditionalFormat {
            background_color: Some(color),
        },
    );
    style
}

#[test]
fn resolve_table_style_finds_a_referenced_style() {
    let mut catalog = StyleCatalog::new();
    catalog.table_styles.insert(
        StyleId::new("Grid"),
        styled("Grid", TableRegion::FirstRow, rgb(1, 2, 3)),
    );
    assert!(resolve_table_style(&catalog, Some("Grid")).is_some());
    assert!(resolve_table_style(&catalog, Some("Missing")).is_none());
    assert!(resolve_table_style(&catalog, None).is_none());
}

#[test]
fn cell_style_shading_applies_the_header_row_under_default_look() {
    // Word's default look enables firstRow, so the header row is shaded.
    let look = TableLook::default();
    let style = styled("Grid", TableRegion::FirstRow, rgb(10, 20, 30));
    assert_eq!(
        cell_style_shading(Some(&style), &look, 0, 1, 4, 3),
        Some(rgb(10, 20, 30))
    );
    // A body cell is not in the first row → no shading from this style.
    assert_eq!(cell_style_shading(Some(&style), &look, 1, 1, 4, 3), None);
}

#[test]
fn tbl_look_with_first_row_off_suppresses_header_shading() {
    // A look with firstRow disabled means the header-row region does not apply.
    let look = TableLook {
        first_row: false,
        ..TableLook::default()
    };
    let style = styled("Grid", TableRegion::FirstRow, rgb(10, 20, 30));
    assert_eq!(cell_style_shading(Some(&style), &look, 0, 1, 4, 3), None);
}

#[test]
fn no_style_means_no_shading() {
    let look = TableLook::default();
    assert_eq!(cell_style_shading(None, &look, 0, 0, 4, 4), None);
}

#[test]
fn table_look_reads_the_encoded_attr_or_defaults() {
    use loki_doc_model::content::table::core::Table;

    // A table with no encoded look → the format default.
    let plain = Table::grid(2, 2);
    assert_eq!(table_look(&plain), TableLook::default());

    // A table carrying an encoded last-row/last-column look decodes to it.
    let mut styled_tbl = Table::grid(2, 2);
    let want = TableLook {
        first_row: false,
        last_row: true,
        first_column: false,
        last_column: true,
        horizontal_banding: false,
        vertical_banding: true,
    };
    styled_tbl.set_table_look_code(Some(want.encode_attr()));
    assert_eq!(table_look(&styled_tbl), want);
}

/// 4a.3: an explicit `w:cnfStyle` mask is authoritative — a cell physically
/// in row 1 that Word stamped `firstRow` still takes the header shading, and
/// a malformed mask falls back to the positional derivation.
#[test]
fn cnf_mask_beats_positional_derivation() {
    use crate::table_shading::cell_style_shading_cnf;
    let style = styled("S", TableRegion::FirstRow, rgb(9, 9, 9));
    let look = TableLook::default();
    // Row 1 positionally gets no header shading …
    assert_eq!(
        cell_style_shading_cnf(Some(&style), &look, None, 1, 0, 3, 3),
        None
    );
    // … but the explicit firstRow mask claims it.
    assert_eq!(
        cell_style_shading_cnf(Some(&style), &look, Some("100000000000"), 1, 0, 3, 3),
        Some(rgb(9, 9, 9))
    );
    // Malformed mask → positional fallback (row 0 IS the header).
    assert_eq!(
        cell_style_shading_cnf(Some(&style), &look, Some("bogus"), 0, 0, 3, 3),
        Some(rgb(9, 9, 9))
    );
}
