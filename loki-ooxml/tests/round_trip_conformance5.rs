// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! OOXML conformance integration tests (part 5).
//!
//! Covers `w:lang @w:bidi` → `CharProps.language_complex` (gap 14) and
//! `w:lang @w:eastAsia` → `CharProps.language_east_asian` (gap 15).

use std::io::{Cursor, Write};
use zip::{CompressionMethod, ZipWriter, write::FileOptions};

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

// ── Shared helpers ────────────────────────────────────────────────────────────

fn deflate() -> FileOptions<'static, ()> {
    FileOptions::<()>::default().compression_method(CompressionMethod::Deflated)
}

fn write_opc_skeleton(zip: &mut ZipWriter<Cursor<&mut Vec<u8>>>, d: FileOptions<'static, ()>) {
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

    zip.start_file("word/_rels/document.xml.rels", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#,
    )
    .unwrap();
}

// ── ooxml14 ───────────────────────────────────────────────────────────────────

/// `w:lang @w:bidi="ar-SA"` must set `CharProps.language_complex = Some("ar-SA")`.
/// [ECMA-376 §17.3.2.20]
#[test]
fn ooxml14_lang_bidi_maps_to_language_complex() {
    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));
    let d = deflate();
    write_opc_skeleton(&mut zip, d);

    zip.start_file("word/document.xml", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r>
        <w:rPr><w:lang w:bidi="ar-SA"/></w:rPr>
        <w:t>Arabic script run.</w:t>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#,
    )
    .unwrap();
    zip.finish().unwrap();

    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf))
        .expect("document with w:lang @w:bidi must import without error");

    let char_props = result
        .document
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .find_map(|b| {
            if let Block::StyledPara(p) = b {
                p.inlines.iter().find_map(|i| {
                    if let Inline::StyledRun(sr) = i {
                        sr.direct_props.as_deref()
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        })
        .expect("StyledRun with direct_props must be present");

    assert_eq!(
        char_props.language_complex.as_ref().map(|t| t.as_str()),
        Some("ar-SA"),
        "w:lang @w:bidi must set language_complex"
    );
}

// ── ooxml15 ───────────────────────────────────────────────────────────────────

/// `w:lang @w:eastAsia="ja-JP"` must set `CharProps.language_east_asian = Some("ja-JP")`.
/// [ECMA-376 §17.3.2.20]
#[test]
fn ooxml15_lang_east_asia_maps_to_language_east_asian() {
    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));
    let d = deflate();
    write_opc_skeleton(&mut zip, d);

    zip.start_file("word/document.xml", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r>
        <w:rPr><w:lang w:eastAsia="ja-JP"/></w:rPr>
        <w:t>Japanese run.</w:t>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#,
    )
    .unwrap();
    zip.finish().unwrap();

    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf))
        .expect("document with w:lang @w:eastAsia must import without error");

    let char_props = result
        .document
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .find_map(|b| {
            if let Block::StyledPara(p) = b {
                p.inlines.iter().find_map(|i| {
                    if let Inline::StyledRun(sr) = i {
                        sr.direct_props.as_deref()
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        })
        .expect("StyledRun with direct_props must be present");

    assert_eq!(
        char_props.language_east_asian.as_ref().map(|t| t.as_str()),
        Some("ja-JP"),
        "w:lang @w:eastAsia must set language_east_asian"
    );
}
