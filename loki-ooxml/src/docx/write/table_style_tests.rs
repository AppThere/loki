// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the table-style writer.

use super::*;
use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::style::catalog::StyleId;
use loki_primitives::color::RgbColor;

fn rgb(r: u8, g: u8, b: u8) -> DocumentColor {
    DocumentColor::Rgb(RgbColor::new(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
    ))
}

fn render<F: FnOnce(&mut Writer<Vec<u8>>)>(f: F) -> String {
    let mut w = Writer::new(Vec::new());
    f(&mut w);
    String::from_utf8(w.into_inner()).unwrap()
}

#[test]
fn writes_conditional_regions_and_band_sizes() {
    let mut style = TableStyle {
        id: StyleId::new("Banded"),
        display_name: Some("Banded".into()),
        parent: None,
        table_props: TableProps {
            row_band_size: Some(2),
            background_color: Some(rgb(255, 255, 255)),
            ..TableProps::default()
        },
        conditional: Default::default(),
        extensions: ExtensionBag::default(),
    };
    style.conditional.insert(
        TableRegion::FirstRow,
        TableConditionalFormat {
            background_color: Some(rgb(0x44, 0x72, 0xC4)),
        },
    );
    let mut catalog = StyleCatalog::new();
    catalog.table_styles.insert(StyleId::new("Banded"), style);

    let xml = render(|w| write_table_styles(w, &catalog));

    assert!(xml.contains(r#"<w:style w:type="table" w:styleId="Banded""#));
    assert!(xml.contains(r#"<w:tblStyleRowBandSize w:val="2"/>"#));
    // Base whole-table shading via tcPr/shd.
    assert!(xml.contains(r#"w:fill="FFFFFF""#));
    // firstRow conditional region with its shading.
    assert!(xml.contains(r#"<w:tblStylePr w:type="firstRow">"#));
    assert!(xml.contains(r#"w:fill="4472C4""#));
}

#[test]
fn a_style_without_bands_omits_tblpr() {
    let style = TableStyle {
        id: StyleId::new("Plain"),
        display_name: None,
        parent: None,
        table_props: TableProps::default(),
        conditional: Default::default(),
        extensions: ExtensionBag::default(),
    };
    let mut catalog = StyleCatalog::new();
    catalog.table_styles.insert(StyleId::new("Plain"), style);

    let xml = render(|w| write_table_styles(w, &catalog));
    assert!(!xml.contains("w:tblPr"));
    assert!(!xml.contains("w:tblStylePr"));
}

#[test]
fn writes_tbl_look_attributes_and_bitmask() {
    // Word default look 04A0: firstRow + firstColumn + horizontal banding.
    let code = TableLook::default().encode_attr();
    let xml = render(|w| write_tbl_look(w, Some(&code)));
    assert!(xml.contains(r#"w:val="04A0""#));
    assert!(xml.contains(r#"w:firstRow="1""#));
    assert!(xml.contains(r#"w:firstColumn="1""#));
    assert!(xml.contains(r#"w:noHBand="0""#)); // horizontal banding on
    assert!(xml.contains(r#"w:noVBand="1""#)); // vertical banding off
    assert!(xml.contains(r#"w:lastRow="0""#));
}

#[test]
fn malformed_tbl_look_code_writes_nothing() {
    let xml = render(|w| write_tbl_look(w, Some("nonsense")));
    assert!(xml.is_empty());
}
