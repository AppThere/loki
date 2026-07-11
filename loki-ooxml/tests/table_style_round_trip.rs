// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! DOCX table-style reference round-trip: a table's named style (`w:tblStyle`,
//! stored in the model as the table's `"style"` attr) must survive export and
//! re-import — the foundation for table banding / conditional formatting
//! (Spec 05, 4a.3).

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::table::core::Table;
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_ooxml::DocxExport;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

fn export_import(doc: &Document) -> Document {
    let mut buf = Cursor::new(Vec::new());
    DocxExport::export(doc, &mut buf, ()).expect("export");
    DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf.into_inner()))
        .expect("re-import")
        .document
}

/// The first `Block::Table` anywhere in the first section's blocks.
fn first_table(doc: &Document) -> Option<&Table> {
    doc.sections[0].blocks.iter().find_map(|b| match b {
        Block::Table(t) => Some(t.as_ref()),
        _ => None,
    })
}

#[test]
fn table_style_reference_round_trips() {
    let mut table = Table::grid(2, 2);
    table.set_style_name(Some("LightGridAccent1".into()));
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];

    let back = export_import(&doc);

    let t = first_table(&back).expect("table survives");
    assert_eq!(t.style_name(), Some("LightGridAccent1"));
}

#[test]
fn a_table_without_a_style_stays_unstyled() {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Table(Box::new(Table::grid(2, 1)))];

    let back = export_import(&doc);

    assert_eq!(first_table(&back).and_then(Table::style_name), None);
}

#[test]
fn table_style_banding_and_tbllook_round_trip() {
    use loki_doc_model::style::catalog::StyleId;
    use loki_doc_model::style::table_style::{
        TableConditionalFormat, TableLook, TableProps, TableRegion, TableStyle,
    };
    use loki_primitives::color::{DocumentColor, RgbColor};

    let blue = DocumentColor::Rgb(RgbColor::new(
        0x44 as f32 / 255.0,
        0x72 as f32 / 255.0,
        0xC4 as f32 / 255.0,
    ));

    // A banded table style in the catalog.
    let mut style = TableStyle {
        id: StyleId::new("Banded"),
        display_name: Some("Banded".into()),
        parent: None,
        table_props: TableProps {
            row_band_size: Some(2),
            ..TableProps::default()
        },
        conditional: Default::default(),
        extensions: Default::default(),
    };
    style.conditional.insert(
        TableRegion::FirstRow,
        TableConditionalFormat {
            background_color: Some(blue),
        },
    );

    // A table referencing the style, with a non-default look (last row/col on).
    let mut table = Table::grid(2, 2);
    table.set_style_name(Some("Banded".into()));
    let look = TableLook {
        last_row: true,
        last_column: true,
        ..TableLook::default()
    };
    table.set_table_look_code(Some(look.encode_attr()));

    let mut doc = Document::new();
    doc.styles
        .table_styles
        .insert(StyleId::new("Banded"), style);
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];

    let back = export_import(&doc);

    // The style definition's conditional shading survives.
    let ts = back
        .styles
        .table_styles
        .get(&StyleId::new("Banded"))
        .expect("table style survives");
    assert_eq!(ts.table_props.row_band_size, Some(2));
    let fr = ts
        .conditional
        .get(&TableRegion::FirstRow)
        .and_then(|c| c.background_color.as_ref())
        .expect("firstRow shading survives");
    assert_eq!(fr.to_hex().as_deref(), Some("#4472C4"));

    // The instance's tblLook survives.
    let t = first_table(&back).expect("table survives");
    let back_look =
        TableLook::decode_attr(t.table_look_code().expect("tbllook present")).expect("decodes");
    assert_eq!(back_look, look);
}
