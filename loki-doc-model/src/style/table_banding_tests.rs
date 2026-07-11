// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the pure table-banding shading resolver.

use super::*;
use crate::content::attr::ExtensionBag;
use crate::style::catalog::StyleId;
use crate::style::table_style::{TableConditionalFormat, TableProps};
use indexmap::IndexMap;
use loki_primitives::color::{DocumentColor, RgbColor};

fn rgb(r: u8, g: u8, b: u8) -> DocumentColor {
    DocumentColor::Rgb(RgbColor::new(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
    ))
}

/// A style whose given regions each carry a distinct shading color.
fn style_with(regions: &[(TableRegion, DocumentColor)]) -> TableStyle {
    let mut conditional = IndexMap::new();
    for (region, color) in regions {
        conditional.insert(
            *region,
            TableConditionalFormat {
                background_color: Some(color.clone()),
            },
        );
    }
    TableStyle {
        id: StyleId("S".into()),
        display_name: None,
        parent: None,
        table_props: TableProps::default(),
        conditional,
        extensions: ExtensionBag::default(),
    }
}

#[test]
fn no_conditional_formatting_yields_no_shading() {
    let style = style_with(&[]);
    let look = TableLook::default();
    assert_eq!(resolve_cell_shading(&style, &look, 1, 1, 4, 4), None);
}

#[test]
fn base_table_shading_is_the_fallback() {
    let mut style = style_with(&[]);
    style.table_props.background_color = Some(rgb(1, 2, 3));
    let look = TableLook::default();
    assert_eq!(
        resolve_cell_shading(&style, &look, 1, 1, 4, 4),
        Some(rgb(1, 2, 3))
    );
}

#[test]
fn horizontal_bands_alternate_below_the_header() {
    // firstRow + horizontal banding on (Word default), band size 1.
    let style = style_with(&[
        (TableRegion::Band1Horz, rgb(10, 0, 0)),
        (TableRegion::Band2Horz, rgb(20, 0, 0)),
    ]);
    let look = TableLook::default();
    // Row 0 is the header — no band.
    assert_eq!(resolve_cell_shading(&style, &look, 0, 1, 5, 3), None);
    // Rows 1,2,3,4 → band index 0,1,2,3 → band1,band2,band1,band2.
    assert_eq!(
        resolve_cell_shading(&style, &look, 1, 1, 5, 3),
        Some(rgb(10, 0, 0))
    );
    assert_eq!(
        resolve_cell_shading(&style, &look, 2, 1, 5, 3),
        Some(rgb(20, 0, 0))
    );
    assert_eq!(
        resolve_cell_shading(&style, &look, 3, 1, 5, 3),
        Some(rgb(10, 0, 0))
    );
}

#[test]
fn row_band_size_groups_rows() {
    let mut style = style_with(&[
        (TableRegion::Band1Horz, rgb(10, 0, 0)),
        (TableRegion::Band2Horz, rgb(20, 0, 0)),
    ]);
    style.table_props.row_band_size = Some(2);
    // No first/last row so banding starts at row 0.
    let look = TableLook {
        first_row: false,
        horizontal_banding: true,
        ..TableLook::default()
    };
    // Rows 0,1 → band 0 (band1); rows 2,3 → band 1 (band2).
    assert_eq!(
        resolve_cell_shading(&style, &look, 0, 0, 6, 3),
        Some(rgb(10, 0, 0))
    );
    assert_eq!(
        resolve_cell_shading(&style, &look, 1, 0, 6, 3),
        Some(rgb(10, 0, 0))
    );
    assert_eq!(
        resolve_cell_shading(&style, &look, 2, 0, 6, 3),
        Some(rgb(20, 0, 0))
    );
}

#[test]
fn first_row_outranks_bands() {
    let style = style_with(&[
        (TableRegion::FirstRow, rgb(99, 0, 0)),
        (TableRegion::Band1Horz, rgb(10, 0, 0)),
    ]);
    let look = TableLook::default();
    assert_eq!(
        resolve_cell_shading(&style, &look, 0, 1, 4, 3),
        Some(rgb(99, 0, 0))
    );
}

#[test]
fn corner_cell_outranks_first_row_and_first_column() {
    let style = style_with(&[
        (TableRegion::NwCell, rgb(1, 1, 1)),
        (TableRegion::FirstRow, rgb(2, 2, 2)),
        (TableRegion::FirstColumn, rgb(3, 3, 3)),
    ]);
    let look = TableLook {
        first_column: true,
        ..TableLook::default()
    };
    // (0,0) is the NW corner.
    assert_eq!(
        resolve_cell_shading(&style, &look, 0, 0, 4, 4),
        Some(rgb(1, 1, 1))
    );
    // (0,1) is first row but not first column → FirstRow.
    assert_eq!(
        resolve_cell_shading(&style, &look, 0, 1, 4, 4),
        Some(rgb(2, 2, 2))
    );
    // (1,0) is first column but not first row → FirstColumn.
    assert_eq!(
        resolve_cell_shading(&style, &look, 1, 0, 4, 4),
        Some(rgb(3, 3, 3))
    );
}

#[test]
fn corner_needs_both_look_flags() {
    // NwCell defined, but first_column flag is off → the corner region does
    // not apply, so a first-row cell resolves to FirstRow instead.
    let style = style_with(&[
        (TableRegion::NwCell, rgb(1, 1, 1)),
        (TableRegion::FirstRow, rgb(2, 2, 2)),
    ]);
    let look = TableLook {
        first_column: false,
        ..TableLook::default()
    };
    assert_eq!(
        resolve_cell_shading(&style, &look, 0, 0, 4, 4),
        Some(rgb(2, 2, 2))
    );
}

#[test]
fn banding_flag_off_suppresses_bands() {
    let style = style_with(&[(TableRegion::Band1Horz, rgb(10, 0, 0))]);
    let look = TableLook {
        horizontal_banding: false,
        ..TableLook::default()
    };
    assert_eq!(resolve_cell_shading(&style, &look, 1, 1, 4, 3), None);
}

#[test]
fn last_row_and_last_column_regions() {
    let style = style_with(&[
        (TableRegion::LastRow, rgb(5, 0, 0)),
        (TableRegion::LastColumn, rgb(0, 5, 0)),
        (TableRegion::SeCell, rgb(0, 0, 5)),
    ]);
    let look = TableLook {
        last_row: true,
        last_column: true,
        ..TableLook::default()
    };
    assert_eq!(
        resolve_cell_shading(&style, &look, 3, 1, 4, 4),
        Some(rgb(5, 0, 0))
    );
    assert_eq!(
        resolve_cell_shading(&style, &look, 1, 3, 4, 4),
        Some(rgb(0, 5, 0))
    );
    // (3,3) is the SE corner.
    assert_eq!(
        resolve_cell_shading(&style, &look, 3, 3, 4, 4),
        Some(rgb(0, 0, 5))
    );
}

#[test]
fn vertical_bands_alternate_between_columns() {
    let style = style_with(&[
        (TableRegion::Band1Vert, rgb(0, 10, 0)),
        (TableRegion::Band2Vert, rgb(0, 20, 0)),
    ]);
    let look = TableLook {
        first_column: true,
        vertical_banding: true,
        horizontal_banding: false,
        ..TableLook::default()
    };
    // Col 0 is the first column — no vertical band.
    assert_eq!(resolve_cell_shading(&style, &look, 1, 0, 4, 5), None);
    // Cols 1,2,3,4 → band 0,1,2,3 → band1,band2,band1,band2.
    assert_eq!(
        resolve_cell_shading(&style, &look, 1, 1, 4, 5),
        Some(rgb(0, 10, 0))
    );
    assert_eq!(
        resolve_cell_shading(&style, &look, 1, 2, 4, 5),
        Some(rgb(0, 20, 0))
    );
}

#[test]
fn horizontal_bands_outrank_vertical_bands() {
    let style = style_with(&[
        (TableRegion::Band1Horz, rgb(10, 0, 0)),
        (TableRegion::Band1Vert, rgb(0, 10, 0)),
    ]);
    let look = TableLook {
        first_row: false,
        first_column: false,
        vertical_banding: true,
        horizontal_banding: true,
        ..TableLook::default()
    };
    // (0,0): horiz band 0 (band1) and vert band 0 (band1) — horiz wins.
    assert_eq!(
        resolve_cell_shading(&style, &look, 0, 0, 4, 4),
        Some(rgb(10, 0, 0))
    );
}

#[test]
fn out_of_range_indices_yield_none() {
    let mut style = style_with(&[]);
    style.table_props.background_color = Some(rgb(1, 2, 3));
    let look = TableLook::default();
    assert_eq!(resolve_cell_shading(&style, &look, 4, 0, 4, 4), None);
    assert_eq!(resolve_cell_shading(&style, &look, 0, 4, 4, 4), None);
    assert_eq!(resolve_cell_shading(&style, &look, 0, 0, 0, 0), None);
}
