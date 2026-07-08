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
    let style = styled("Grid", TableRegion::FirstRow, rgb(10, 20, 30));
    assert_eq!(
        cell_style_shading(Some(&style), 0, 1, 4, 3),
        Some(rgb(10, 20, 30))
    );
    // A body cell is not in the first row → no shading from this style.
    assert_eq!(cell_style_shading(Some(&style), 1, 1, 4, 3), None);
}

#[test]
fn no_style_means_no_shading() {
    assert_eq!(cell_style_shading(None, 0, 0, 4, 4), None);
}
