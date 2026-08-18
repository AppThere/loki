// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Literal line feeds inside `w:t` are whitespace, not line breaks.
//!
//! A line break in OOXML is `<w:br/>`. A generator that pretty-prints a code
//! block or a directory tree into a single `<w:t xml:space="preserve">` leaves
//! real LF characters in the character data, and Word renders each as **one
//! space**, wrapping the run normally. Carrying them through as breaks turns
//! one wrapped paragraph into as many lines as it has feeds: in
//! `iris-blueprint.docx` a 12-line tree became 21 lines, overflowing page 9 and
//! pushing the whole "8. Technology Stack" table onto the next page.

use std::io::{Cursor, Write};

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipWriter};

/// A DOCX whose one paragraph holds a `w:t` with literal feeds *and* an
/// explicit `<w:br/>`, so the two cannot be conflated.
fn docx_with_literal_newlines() -> Vec<u8> {
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
    // The feed sits between "tree/" and the two indent spaces before "sub", so
    // the rendered gap is three spaces — exactly what Word shows.
    zip.write_all(
        b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\n\
  <w:body>\n\
    <w:p><w:r><w:t xml:space=\"preserve\">tree/\n  sub/\n  leaf</w:t></w:r></w:p>\n\
    <w:p><w:r><w:t>before</w:t><w:br/><w:t>after</w:t></w:r></w:p>\n\
  </w:body>\n\
</w:document>",
    )
    .unwrap();

    zip.finish().unwrap();
    buf
}

/// Flattens one block's inlines, marking an `Inline::LineBreak` as `\n` so a
/// break and a space are distinguishable in the result.
fn flatten(doc: &Document, block_index: usize) -> String {
    fn walk(inlines: &[Inline], out: &mut String) {
        for i in inlines {
            match i {
                Inline::Str(s) => out.push_str(s),
                Inline::Space => out.push(' '),
                Inline::LineBreak => out.push('\n'),
                Inline::StyledRun(r) => walk(&r.content, out),
                Inline::Emph(c) | Inline::Strong(c) | Inline::Span(_, c) => walk(c, out),
                _ => {}
            }
        }
    }
    let mut out = String::new();
    let block = doc
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .nth(block_index)
        .expect("block should exist");
    match block {
        Block::StyledPara(sp) => walk(&sp.inlines, &mut out),
        Block::Para(i) | Block::Plain(i) => walk(i, &mut out),
        _ => panic!("expected a paragraph"),
    }
    out
}

#[test]
fn literal_newlines_in_wt_become_spaces_not_breaks() {
    let doc = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(docx_with_literal_newlines()))
        .expect("import should succeed")
        .document;

    let got = flatten(&doc, 0);

    // One space per feed, and the run's own indentation kept: "tree/" + the
    // feed's space + two indent spaces = three spaces. Asserting the whole
    // string pins all three of the ways this can go wrong — a break, a dropped
    // feed, and a collapsed run of whitespace.
    assert_eq!(
        got, "tree/   sub/   leaf",
        "each literal feed must render as exactly one space, preserving the \
         surrounding indentation"
    );
    assert!(
        !got.contains('\n'),
        "a literal feed must not become a line break: {got:?}"
    );

    // The inverse: an explicit `<w:br/>` in the same document *must* still be a
    // break. Without this the fix could be \"strip every newline\" and pass.
    let with_br = flatten(&doc, 1);
    assert_eq!(
        with_br, "before\nafter",
        "an explicit <w:br/> must still produce a line break"
    );
}
