// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! OOXML conformance integration tests (part 2) derived from [MS-OI29500] and [MS-DOCX].
//!
//! Covers Strict namespace support, `w:rFonts` font-name storage, and silent
//! `w:sdt` drop behaviour.

use std::io::{Cursor, Write};

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

// ── Shared mini-DOCX builder ─────────────────────────────────────────────────

/// Writes `[Content_Types].xml`, `_rels/.rels` (Transitional), and
/// `word/_rels/document.xml.rels` into `zip`.
fn write_minimal_opc_skeleton(
    zip: &mut zip::ZipWriter<Cursor<&mut Vec<u8>>>,
    d: zip::write::FileOptions<()>,
) {
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

// ── Tests ────────────────────────────────────────────────────────────────────

/// A DOCX package whose `_rels/.rels` uses the ISO Strict relationship URI
/// (`http://purl.oclc.org/ooxml/officeDocument/relationships/officeDocument`)
/// instead of the Transitional URI must import without error. [MS-OI29500] §1.
#[test]
fn ooxml6_strict_namespace_imports_correctly() {
    use zip::{CompressionMethod, ZipWriter, write::FileOptions};

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

    // Strict profile: purl.oclc.org relationship type URI.
    zip.start_file("_rels/.rels", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1"
    Type="http://purl.oclc.org/ooxml/officeDocument/relationships/officeDocument"
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

    zip.start_file("word/document.xml", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Strict namespace document.</w:t></w:r></w:p>
  </w:body>
</w:document>"#,
    )
    .unwrap();

    zip.finish().unwrap();

    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf))
        .expect("Strict OOXML package must import without error");

    let all_blocks: Vec<&Block> = result
        .document
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .collect();

    assert!(
        !all_blocks.is_empty(),
        "Strict OOXML package must produce at least one block"
    );
}

/// `w:rFonts` stores the `w:ascii` font under `font_name` and the `w:cs`
/// (complex-script) font under `font_name_complex` as separate fields.
/// [MS-OI29500] §2.1 Rule 1: the cs font is used for RTL/complex-script runs.
#[test]
fn ooxml7_rfonts_ascii_and_cs_stored_separately() {
    use zip::{CompressionMethod, ZipWriter, write::FileOptions};

    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));
    let d = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);

    write_minimal_opc_skeleton(&mut zip, d);

    zip.start_file("word/document.xml", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r>
        <w:rPr>
          <w:rFonts w:ascii="Times New Roman" w:cs="Arial"/>
        </w:rPr>
        <w:t>Font test.</w:t>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#,
    )
    .unwrap();

    zip.finish().unwrap();

    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf))
        .expect("rFonts document must import without error");

    let all_blocks: Vec<&Block> = result
        .document
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .collect();

    let props = all_blocks
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
        .expect("styled run with direct char props must be present");

    assert_eq!(
        props.font_name.as_deref(),
        Some("Times New Roman"),
        "w:ascii font must be stored in font_name"
    );
    assert_eq!(
        props.font_name_complex.as_deref(),
        Some("Arial"),
        "w:cs font must be stored in font_name_complex"
    );
}

/// A block-level `<w:sdt>` (content control) between two body paragraphs is
/// **unwrapped** by the importer (5.9): its `w:sdtContent` children survive as
/// normal body blocks rather than being dropped. [MS-DOCX] §2.2.
#[test]
fn ooxml8_sdt_content_between_paragraphs_is_unwrapped() {
    use zip::{CompressionMethod, ZipWriter, write::FileOptions};

    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));
    let d = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);

    write_minimal_opc_skeleton(&mut zip, d);

    zip.start_file("word/document.xml", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Before SDT.</w:t></w:r></w:p>
    <w:sdt>
      <w:sdtPr/>
      <w:sdtContent>
        <w:p><w:r><w:t>SDT content paragraph.</w:t></w:r></w:p>
      </w:sdtContent>
    </w:sdt>
    <w:p><w:r><w:t>After SDT.</w:t></w:r></w:p>
  </w:body>
</w:document>"#,
    )
    .unwrap();

    zip.finish().unwrap();

    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf))
        .expect("document with w:sdt must import without panic");

    let all_blocks: Vec<&Block> = result
        .document
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .collect();

    // Before + the unwrapped SDT content paragraph + After — nothing dropped.
    assert_eq!(
        all_blocks.len(),
        3,
        "the SDT content paragraph must be unwrapped, not dropped"
    );
    let text: String = all_blocks
        .iter()
        .flat_map(|b| match b {
            Block::Para(inl) | Block::Plain(inl) => inl.clone(),
            Block::StyledPara(sp) => sp.inlines.clone(),
            _ => Vec::new(),
        })
        .filter_map(|i| match i {
            loki_doc_model::content::inline::Inline::Str(s) => Some(s),
            _ => None,
        })
        .collect();
    assert!(
        text.contains("SDT content paragraph"),
        "the control's text must survive: {text}"
    );
}
