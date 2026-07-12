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
                char_props: Default::default(),
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

/// 4a.3: an explicit `w:cnfStyle` mask resolves regions directly — no
/// positional/look derivation — with the same precedence order.
#[test]
fn cnf_mask_resolves_regions_without_position() {
    use crate::style::table_cnf::TableCnf;
    let style = style_with(&[
        (TableRegion::FirstRow, rgb(1, 0, 0)),
        (TableRegion::Band1Horz, rgb(0, 1, 0)),
    ]);
    // firstRow + band1Horz mask: firstRow outranks the band.
    let cnf = TableCnf::decode_attr("100000100000").unwrap();
    assert_eq!(
        resolve_cell_shading_cnf(&style, &cnf),
        Some(rgb(1, 0, 0)),
        "firstRow wins by precedence"
    );
    // band1Horz-only mask picks the band shading.
    let cnf = TableCnf::decode_attr("000000100000").unwrap();
    assert_eq!(resolve_cell_shading_cnf(&style, &cnf), Some(rgb(0, 1, 0)));
    // A mask claiming no shaded region falls back to the base shading (none).
    let cnf = TableCnf::decode_attr("000000000000").unwrap();
    assert_eq!(resolve_cell_shading_cnf(&style, &cnf), None);
}

/// 4a.3: region character formatting merges low→high precedence — the
/// firstRow rPr overrides the wholeTable rPr per property, and untouched
/// properties fall through.
#[test]
fn region_char_props_merge_by_precedence() {
    use crate::style::props::char_props::CharProps;
    use crate::style::table_banding::resolve_cell_char_props;
    use loki_primitives::units::Points;

    let mut style = style_with(&[]);
    style.conditional.insert(
        TableRegion::WholeTable,
        TableConditionalFormat {
            background_color: None,
            char_props: CharProps {
                font_size: Some(Points::new(10.0)),
                italic: Some(true),
                ..Default::default()
            },
        },
    );
    style.conditional.insert(
        TableRegion::FirstRow,
        TableConditionalFormat {
            background_color: None,
            char_props: CharProps {
                font_size: Some(Points::new(20.0)),
                bold: Some(true),
                ..Default::default()
            },
        },
    );
    let look = TableLook::default();
    // Header cell: firstRow size wins, wholeTable italic falls through.
    let hdr = resolve_cell_char_props(&style, &look, 0, 0, 3, 3).expect("header chars");
    assert_eq!(hdr.font_size, Some(Points::new(20.0)));
    assert_eq!(hdr.bold, Some(true));
    assert_eq!(hdr.italic, Some(true), "wholeTable property falls through");
    // Body cell: only wholeTable applies.
    let body = resolve_cell_char_props(&style, &look, 2, 1, 3, 3).expect("body chars");
    assert_eq!(body.font_size, Some(Points::new(10.0)));
    assert_eq!(body.bold, None);
}
