// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! OOXML conformance integration tests (part 4).
//!
//! Covers `w:outline` run toggle → `CharProps.outline` (gap 5) and
//! `w:pPr/w:rPr` → `StyledParagraph.direct_char_props` (gap 6).

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

// ── ooxml12 ───────────────────────────────────────────────────────────────────

/// `<w:outline/>` in `w:rPr` must set `CharProps.outline = Some(true)` on the
/// resulting `StyledRun`. [ECMA-376 §17.3.2.23]
#[test]
fn ooxml12_rpr_outline_maps_to_char_props() {
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
        <w:rPr><w:outline/></w:rPr>
        <w:t>Outline text.</w:t>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#,
    )
    .unwrap();
    zip.finish().unwrap();

    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf))
        .expect("run with w:outline must import without error");

    let all_blocks: Vec<&Block> = result
        .document
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .collect();

    let char_props = all_blocks
        .iter()
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
        char_props.outline,
        Some(true),
        "CharProps.outline must be Some(true) after w:outline toggle"
    );
}

// ── ooxml13 ───────────────────────────────────────────────────────────────────

/// `w:pPr/w:rPr` paragraph-mark run properties must be mapped to
/// `StyledParagraph.direct_char_props`. [ECMA-376 §17.3.1.26 §17.3.2.28]
#[test]
fn ooxml13_ppr_rpr_maps_to_direct_char_props() {
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
      <w:pPr>
        <w:rPr><w:b/><w:color w:val="FF0000"/></w:rPr>
      </w:pPr>
      <w:r><w:t>Paragraph with mark formatting.</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#,
    )
    .unwrap();
    zip.finish().unwrap();

    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf))
        .expect("paragraph with w:pPr/w:rPr must import without error");

    let all_blocks: Vec<&Block> = result
        .document
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .collect();

    let direct_char = match all_blocks[0] {
        Block::StyledPara(sp) => sp
            .direct_char_props
            .as_deref()
            .expect("direct_char_props must be Some when w:pPr/w:rPr is present"),
        other => panic!("expected StyledPara, got {other:?}"),
    };

    assert_eq!(
        direct_char.bold,
        Some(true),
        "paragraph-mark bold must be captured in direct_char_props"
    );

    let hex = direct_char
        .color
        .as_ref()
        .expect("paragraph-mark color must be Some")
        .to_hex()
        .expect("color must be representable as hex");
    assert_eq!(
        hex.trim_start_matches('#').to_ascii_uppercase(),
        "FF0000",
        "paragraph-mark color must round-trip as FF0000, got {hex}"
    );
}
