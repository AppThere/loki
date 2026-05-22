// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! OOXML conformance integration tests (part 3).
//!
//! Covers paragraph background colour (`w:pPr/w:shd`), run background colour
//! (`w:rPr/w:shd`), and column-break promotion to `ParaProps.column_break_after`.

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

// ── ooxml9 ────────────────────────────────────────────────────────────────────

/// `w:pPr/w:shd @w:fill` must set `ParaProps.background_color` on the
/// resulting `Block::StyledPara`. [ECMA-376 §17.3.1.24 / §17.3.5.44]
#[test]
fn ooxml9_paragraph_shading_maps_to_background_color() {
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
        <w:shd w:val="clear" w:color="auto" w:fill="FF0000"/>
      </w:pPr>
      <w:r><w:t>Red background paragraph.</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#,
    )
    .unwrap();
    zip.finish().unwrap();

    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf))
        .expect("paragraph with w:shd must import without error");

    let all_blocks: Vec<&Block> = result
        .document
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .collect();

    let props = match all_blocks[0] {
        Block::StyledPara(sp) => sp
            .direct_para_props
            .as_deref()
            .expect("direct_para_props must be Some when w:shd is present"),
        other => panic!("expected StyledPara, got {other:?}"),
    };

    let color = props
        .background_color
        .as_ref()
        .expect("background_color must be Some after w:pPr/w:shd FF0000");

    let hex = color.to_hex().expect("color must be representable as hex");
    let hex = hex.trim_start_matches('#');
    assert_eq!(
        hex.to_ascii_uppercase(),
        "FF0000",
        "paragraph background color must round-trip as FF0000, got {hex}"
    );
}

// ── ooxml10 ───────────────────────────────────────────────────────────────────

/// `w:rPr/w:shd @w:fill` must set `CharProps.background_color` on the
/// resulting `StyledRun`. [ECMA-376 §17.3.2.32]
#[test]
fn ooxml10_run_shading_maps_to_background_color() {
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
        <w:rPr>
          <w:shd w:val="clear" w:color="auto" w:fill="00FF00"/>
        </w:rPr>
        <w:t>Green background run.</w:t>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#,
    )
    .unwrap();
    zip.finish().unwrap();

    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf))
        .expect("run with w:shd must import without error");

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

    let color = char_props
        .background_color
        .as_ref()
        .expect("background_color must be Some after w:rPr/w:shd 00FF00");

    let hex = color.to_hex().expect("color must be representable as hex");
    let hex = hex.trim_start_matches('#');
    assert_eq!(
        hex.to_ascii_uppercase(),
        "00FF00",
        "run background color must round-trip as 00FF00, got {hex}"
    );
}

// ── ooxml11 ───────────────────────────────────────────────────────────────────

/// `<w:br w:type="column"/>` must be promoted to `ParaProps.column_break_after`
/// on the containing paragraph, analogous to how page breaks are promoted to
/// `page_break_after`. [ECMA-376 §17.3.3]
#[test]
fn ooxml11_column_break_promoted_to_para_props() {
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
      <w:r><w:t>Before column break.</w:t></w:r>
      <w:r><w:br w:type="column"/></w:r>
    </w:p>
    <w:p>
      <w:r><w:t>After column break.</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#,
    )
    .unwrap();
    zip.finish().unwrap();

    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf))
        .expect("document with w:br type=column must import without error");

    let all_blocks: Vec<&Block> = result
        .document
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .collect();

    assert_eq!(all_blocks.len(), 2, "must produce exactly two paragraphs");

    let first_props = match all_blocks[0] {
        Block::StyledPara(sp) => sp.direct_para_props.as_deref(),
        other => panic!("expected StyledPara, got {other:?}"),
    };

    assert_eq!(
        first_props.and_then(|pp| pp.column_break_after),
        Some(true),
        "first paragraph must have column_break_after=true"
    );

    let second_props = match all_blocks[1] {
        Block::StyledPara(sp) => sp.direct_para_props.as_deref(),
        other => panic!("expected StyledPara, got {other:?}"),
    };

    assert_eq!(
        second_props.and_then(|pp| pp.column_break_after),
        None,
        "second paragraph must not have column_break_after"
    );
}
