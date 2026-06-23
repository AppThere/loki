// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `w:fldSimple` (simple field) import + round-trip.
//!
//! Word may write a field in the compact `w:fldSimple` form instead of the
//! `w:fldChar`/`w:instrText` complex form. The importer must read it, and a
//! full export→re-import must preserve the field (our exporter always emits
//! the complex form, so the second import exercises that path too).

use std::io::{Cursor, Write};

use loki_doc_model::content::block::Block;
use loki_doc_model::content::field::types::FieldKind;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentExport;
use loki_ooxml::DocxExport;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Builds a minimal DOCX whose single paragraph holds two simple fields:
/// a `PAGE` field with a cached result of "3", and a self-closing `TITLE`.
fn fld_simple_docx() -> Vec<u8> {
    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));
    let d = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);

    zip.start_file("[Content_Types].xml", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml"
    ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
    )
    .unwrap();

    zip.start_file("_rels/.rels", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
    Target="word/document.xml"/>
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("word/document.xml", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:t xml:space="preserve">Page </w:t></w:r>
      <w:fldSimple w:instr=" PAGE ">
        <w:r><w:t>3</w:t></w:r>
      </w:fldSimple>
      <w:fldSimple w:instr=" TITLE "/>
    </w:p>
  </w:body>
</w:document>"#,
    )
    .unwrap();

    zip.finish().unwrap();
    buf
}

/// Collects every field in document order.
fn fields(doc: &Document) -> Vec<(FieldKind, Option<String>)> {
    fn walk(inlines: &[Inline], out: &mut Vec<(FieldKind, Option<String>)>) {
        for i in inlines {
            match i {
                Inline::Field(f) => out.push((f.kind.clone(), f.current_value.clone())),
                Inline::StyledRun(r) => walk(&r.content, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    for section in &doc.sections {
        for block in &section.blocks {
            let inlines = match block {
                Block::Para(i) | Block::Plain(i) => i.as_slice(),
                Block::StyledPara(sp) => sp.inlines.as_slice(),
                _ => &[],
            };
            walk(inlines, &mut out);
        }
    }
    out
}

#[test]
fn simple_field_is_imported() {
    let doc = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(fld_simple_docx()))
        .expect("import should succeed")
        .document;

    let got = fields(&doc);
    assert_eq!(
        got,
        vec![
            (FieldKind::PageNumber, Some("3".to_string())),
            (FieldKind::Title, None),
        ],
        "w:fldSimple fields must import as Inline::Field with their cached value"
    );
}

#[test]
fn simple_field_survives_round_trip() {
    let imported = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(fld_simple_docx()))
        .expect("import should succeed")
        .document;

    // Export (writes the complex form) and re-import.
    let mut out = Cursor::new(Vec::new());
    DocxExport::export(&imported, &mut out, ()).expect("export should succeed");
    let reimported = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(out.into_inner()))
        .expect("re-import should succeed")
        .document;

    assert_eq!(
        fields(&reimported),
        vec![
            (FieldKind::PageNumber, Some("3".to_string())),
            (FieldKind::Title, None),
        ],
        "simple-field semantics must survive a full export/re-import"
    );
}
