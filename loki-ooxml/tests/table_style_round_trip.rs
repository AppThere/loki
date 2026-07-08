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
