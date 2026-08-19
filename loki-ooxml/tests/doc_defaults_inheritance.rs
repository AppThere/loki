// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `w:docDefaults` reach a style that declares no `w:basedOn`.
//!
//! They are the lowest level of the style hierarchy (ECMA-376 §17.7.2), not a
//! fallback for documents that omit `Normal`. Parenting only the *synthesised*
//! `Normal` to them left every real, root-level style resolving against engine
//! defaults instead of the document's — and Word writes both a `docDefaults`
//! and an explicit `Normal` in essentially every document it saves.
//!
//! It hid in `iris-blueprint.docx` twice over: its runs carry explicit
//! `w:rFonts`, and its `docDefaults` font is Arial, which is also Loki's own
//! fallback. `acid2-docx.docx` declares `Calibri` at 22 half-points with a bare
//! `Normal`, and body text came out 12 pt Arial — 9 % wider than Word, so every
//! line broke early and the whole page drifted.

use std::io::{Cursor, Write};

use loki_doc_model::content::block::Block;
use loki_doc_model::document::Document;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipWriter};

/// A DOCX whose `docDefaults` name a font and size, with `styles_extra` spliced
/// in after them so each test can choose what `Normal` looks like.
fn docx_with(styles_extra: &str, para: &str) -> Vec<u8> {
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
  <Override PartName="/word/styles.xml"
    ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
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
  <Relationship Id="rId1"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles"
    Target="styles.xml"/>
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("word/styles.xml", d).unwrap();
    zip.write_all(
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:docDefaults><w:rPrDefault><w:rPr>
    <w:rFonts w:ascii="Cambria" w:hAnsi="Cambria" w:cs="Cambria"/>
    <w:sz w:val="26"/>
  </w:rPr></w:rPrDefault></w:docDefaults>
  {styles_extra}
</w:styles>"#
        )
        .as_bytes(),
    )
    .unwrap();

    zip.start_file("word/document.xml", d).unwrap();
    zip.write_all(
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>{para}</w:body>
</w:document>"#
        )
        .as_bytes(),
    )
    .unwrap();

    zip.finish().unwrap();
    buf
}

/// The `(font, size)` the first run of the first paragraph resolves to.
fn first_run_face(doc: &Document) -> (Option<String>, f32) {
    let block = doc
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .find(|b| matches!(b, Block::StyledPara(_)))
        .expect("a paragraph");
    let Block::StyledPara(p) = block else {
        unreachable!()
    };
    let (_t, spans, _i, _n) = loki_layout::resolve::flatten_paragraph(p, &doc.styles, &mut 0u32);
    let s = spans.first().expect("a span");
    (s.font_name.clone(), s.font_size)
}

fn import(styles_extra: &str, para: &str) -> Document {
    DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(docx_with(styles_extra, para)))
        .expect("import should succeed")
        .document
}

const PLAIN: &str = "<w:p><w:r><w:t>body</w:t></w:r></w:p>";

#[test]
fn doc_defaults_reach_a_style_with_no_based_on() {
    // The regressing shape: an explicit `Normal` with no `w:basedOn`. Word
    // writes this in almost every document.
    let doc = import(
        r#"<w:style w:type="paragraph" w:styleId="Normal"><w:name w:val="Normal"/></w:style>"#,
        PLAIN,
    );
    assert_eq!(
        first_run_face(&doc),
        (Some("Cambria".to_string()), 13.0),
        "a bare `Normal` must still inherit docDefaults (Cambria, 26 half-points = 13pt)"
    );

    // Inversion 1: the *absent*-`Normal` path already worked, so it cannot be
    // what makes the assertion above pass.
    let doc = import("", PLAIN);
    assert_eq!(
        first_run_face(&doc),
        (Some("Cambria".to_string()), 13.0),
        "a synthesised `Normal` must inherit docDefaults too"
    );

    // Inversion 2: an explicit `w:basedOn` must still win — the fix must not
    // reparent every style onto docDefaults regardless.
    let doc = import(
        r#"<w:style w:type="paragraph" w:styleId="Base"><w:name w:val="Base"/>
             <w:rPr><w:rFonts w:ascii="Courier New" w:hAnsi="Courier New"/><w:sz w:val="40"/></w:rPr>
           </w:style>
           <w:style w:type="paragraph" w:styleId="Body"><w:name w:val="Body"/>
             <w:basedOn w:val="Base"/></w:style>"#,
        r#"<w:p><w:pPr><w:pStyle w:val="Body"/></w:pPr><w:r><w:t>body</w:t></w:r></w:p>"#,
    );
    assert_eq!(
        first_run_face(&doc),
        (Some("Courier New".to_string()), 20.0),
        "an explicit `w:basedOn` chain must win over docDefaults"
    );

    // Inversion 3: a run's own `w:rFonts` still beats the inherited default —
    // this is the case `iris-blueprint` is made of, and why it never caught it.
    let doc = import(
        r#"<w:style w:type="paragraph" w:styleId="Normal"><w:name w:val="Normal"/></w:style>"#,
        r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Tinos" w:hAnsi="Tinos"/><w:sz w:val="18"/></w:rPr>
             <w:t>body</w:t></w:r></w:p>"#,
    );
    assert_eq!(
        first_run_face(&doc),
        (Some("Tinos".to_string()), 9.0),
        "direct run formatting must still override the document default"
    );
}
