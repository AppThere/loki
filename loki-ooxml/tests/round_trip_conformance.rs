// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! OOXML conformance integration tests derived from [MS-OI29500] and [MS-DOCX].
//!
//! Each test verifies a specific spec requirement against a hand-crafted
//! minimal DOCX archive built in memory.

mod helpers;

use std::io::{Cursor, Write};

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

// ── Shared mini-DOCX builder ─────────────────────────────────────────────────

/// Writes `[Content_Types].xml`, `_rels/.rels`, and `word/_rels/document.xml.rels`
/// into `zip` using the standard Transitional namespace relationship types.
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

/// Tab stop positions at the maximum values permitted by [MS-OI29500] §2.2
/// (±31 680 twips = ±1 584 pt) must map to correct point values and must not
/// cause a panic or parse failure.
#[test]
fn ooxml3_tab_stop_extreme_positions() {
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
      <w:pPr>
        <w:tabs>
          <w:tab w:val="left"  w:pos="31680"/>
          <w:tab w:val="right" w:pos="-31680"/>
        </w:tabs>
      </w:pPr>
      <w:r><w:t>Extreme tab stop positions.</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#,
    )
    .unwrap();

    zip.finish().unwrap();

    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf))
        .expect("extreme tab stop positions should import without panic");

    let all_blocks: Vec<&Block> = result
        .document
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .collect();

    let tab_stops = all_blocks
        .iter()
        .find_map(|b| {
            if let Block::StyledPara(p) = b {
                p.direct_para_props.as_ref()?.tab_stops.as_ref()
            } else {
                None
            }
        })
        .expect("paragraph with tab stops must be present");

    assert_eq!(tab_stops.len(), 2, "should have exactly 2 tab stops");

    // 31 680 twips ÷ 20 = 1 584.0 pt (spec maximum)
    let max_pos = tab_stops
        .iter()
        .map(|t| t.position.value())
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        (max_pos - 1584.0).abs() < 0.5,
        "positive extreme tab stop should be ~1584 pt (31 680 twips), got {max_pos:.2}"
    );

    // -31 680 twips ÷ 20 = -1 584.0 pt (spec minimum)
    let min_pos = tab_stops
        .iter()
        .map(|t| t.position.value())
        .fold(f64::INFINITY, f64::min);
    assert!(
        (min_pos - (-1584.0)).abs() < 0.5,
        "negative extreme tab stop should be ~-1584 pt (-31 680 twips), got {min_pos:.2}"
    );
}

/// Documents containing [MS-DOCX] §2.1 extension attributes (`w14:paraId`,
/// `w14:textId`) on paragraphs and runs must import without error. These
/// attributes are collaboration metadata that the abstract model does not
/// expose, so they should be silently ignored.
#[test]
fn ooxml4_w14_extension_attributes_ignored_gracefully() {
    use zip::{CompressionMethod, ZipWriter, write::FileOptions};

    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));
    let d = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);

    write_minimal_opc_skeleton(&mut zip, d);

    zip.start_file("word/document.xml", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document
  xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
  xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml">
  <w:body>
    <w:p w14:paraId="3F2A1C5B" w14:textId="12A4B6C8">
      <w:r w14:textId="5D6E7F8A">
        <w:t>Paragraph with extension attributes.</w:t>
      </w:r>
    </w:p>
    <w:p w14:paraId="4A1C3B2E" w14:textId="7F8A9B0C">
      <w:r>
        <w:rPr><w:b/></w:rPr>
        <w:t>Bold paragraph also with extension attributes.</w:t>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#,
    )
    .unwrap();

    zip.finish().unwrap();

    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf))
        .expect("w14 extension attributes must not cause import failure");

    let all_blocks: Vec<&Block> = result
        .document
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .collect();

    assert_eq!(
        all_blocks.len(),
        2,
        "w14 attributes must not alter block count — expected 2 blocks"
    );

    // The second paragraph has a bold run; verify it survives the import.
    let has_bold = all_blocks.iter().any(|b| {
        if let Block::StyledPara(p) = b {
            p.inlines.iter().any(|i| {
                if let Inline::StyledRun(sr) = i {
                    sr.direct_props
                        .as_ref()
                        .is_some_and(|cp| cp.bold == Some(true))
                } else {
                    false
                }
            })
        } else {
            false
        }
    });
    assert!(
        has_bold,
        "bold run must survive import alongside w14 attributes"
    );
}

/// A document that uses `w15:collapsed` on a heading paragraph property must
/// import successfully. Collapsing is a UI state hint that has no effect on
/// the abstract document model. [MS-DOCX] §2.4.
#[test]
fn ooxml5_w15_collapsed_heading_ignored_gracefully() {
    use zip::{CompressionMethod, ZipWriter, write::FileOptions};

    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));
    let d = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);

    write_minimal_opc_skeleton(&mut zip, d);

    zip.start_file("word/document.xml", d).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document
  xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
  xmlns:w15="http://schemas.microsoft.com/office/word/2012/wordml">
  <w:body>
    <w:p>
      <w:pPr>
        <w:pStyle w:val="Heading1"/>
        <w:outlineLvl w:val="0"/>
        <w15:collapsed/>
      </w:pPr>
      <w:r><w:t>Collapsed Heading</w:t></w:r>
    </w:p>
    <w:p>
      <w:r><w:t>Body text after collapsed heading.</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#,
    )
    .unwrap();

    zip.finish().unwrap();

    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(buf))
        .expect("w15:collapsed heading must import without error");

    // The heading and body paragraph are both present.
    let all_blocks: Vec<&Block> = result
        .document
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .collect();

    assert!(
        all_blocks.len() >= 2,
        "document must contain the heading and at least one body block"
    );

    let has_heading = all_blocks
        .iter()
        .any(|b| matches!(b, Block::Heading(1, _, _)));
    assert!(has_heading, "Block::Heading(1, …) must be present");
}
