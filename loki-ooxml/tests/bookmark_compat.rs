// SPDX-License-Identifier: Apache-2.0

use std::fs::File;
use std::io::{BufReader, Cursor, Write};
use zip::CompressionMethod;
use zip::ZipWriter;
use zip::write::FileOptions;

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_ooxml::docx::import::{DocxImportOptions, DocxImporter};

fn flatten_inlines(inlines: &[Inline]) -> String {
    let mut s = String::new();
    for inline in inlines {
        match inline {
            Inline::Str(t) => s.push_str(t),
            Inline::Space => s.push(' '),
            Inline::SoftBreak => s.push(' '),
            Inline::LineBreak => s.push('\n'),
            Inline::Code(_, t) => s.push_str(t),
            Inline::StyledRun(run) => s.push_str(&flatten_inlines(&run.content)),
            Inline::Strong(children) => s.push_str(&flatten_inlines(children)),
            Inline::Emph(children) => s.push_str(&flatten_inlines(children)),
            Inline::Underline(children) => s.push_str(&flatten_inlines(children)),
            Inline::Strikeout(children) => s.push_str(&flatten_inlines(children)),
            Inline::Superscript(children) => s.push_str(&flatten_inlines(children)),
            Inline::Subscript(children) => s.push_str(&flatten_inlines(children)),
            Inline::SmallCaps(children) => s.push_str(&flatten_inlines(children)),
            Inline::Link(_, children, _) => s.push_str(&flatten_inlines(children)),
            _ => {}
        }
    }
    s
}

#[test]
fn test_duplicate_bookmark_ids_dont_corrupt_runs() {
    let mut zip_buf = Vec::new();
    {
        let mut zip = ZipWriter::new(Cursor::new(&mut zip_buf));
        let d = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);

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
      <w:bookmarkStart w:id="1" w:name="TOC"/>
      <w:r><w:t>Hello</w:t></w:r>
      <w:bookmarkEnd w:id="1"/>
    </w:p>
    <w:p>
      <w:bookmarkStart w:id="1" w:name="exec_summary"/>
      <w:r><w:t>World</w:t></w:r>
      <w:bookmarkEnd w:id="1"/>
    </w:p>
  </w:body>
</w:document>"#,
        )
        .unwrap();

        zip.finish().unwrap();
    }

    let result = DocxImporter::new(DocxImportOptions::default())
        .run(Cursor::new(zip_buf))
        .expect("import should succeed");

    assert!(
        result.warnings.is_empty(),
        "should have no warnings: {:?}",
        result.warnings
    );

    let doc = result.document;
    let texts: Vec<String> = doc
        .blocks_flat()
        .filter_map(|b| match b {
            Block::Para(inlines) => Some(flatten_inlines(inlines)),
            Block::StyledPara(sp) => Some(flatten_inlines(&sp.inlines)),
            Block::Plain(inlines) => Some(flatten_inlines(inlines)),
            _ => None,
        })
        .filter(|s| !s.is_empty())
        .collect();

    assert!(
        texts.iter().any(|t| t.contains("Hello")),
        "first paragraph text should be preserved"
    );
    assert!(
        texts.iter().any(|t| t.contains("World")),
        "second paragraph text should be preserved"
    );
}

#[test]
fn test_import_architecture_doc() {
    let file = File::open("tests/AppThere_Loki_Architecture.docx").expect("failed to open file");
    let result = DocxImporter::new(DocxImportOptions::default())
        .run(BufReader::new(file))
        .expect("import failed");

    let doc = result.document;
    let mut bookmark_start_count = 0;
    let mut bookmark_end_count = 0;

    let mut has_exec_summary = false;
    let mut has_goals_nongoals = false;

    // We will collect paragraphs and headings text to verify
    for block in doc.blocks_flat() {
        match block {
            Block::Heading(_lvl, _, inlines) => {
                let text = flatten_inlines(inlines);
                if text.contains("Executive Summary") {
                    has_exec_summary = true;
                }
                if text.contains("Goals & Non-Goals") || text.contains("Goals") {
                    has_goals_nongoals = true;
                }
                for inline in inlines {
                    if let Inline::Bookmark(kind, _name) = inline {
                        if *kind == loki_doc_model::content::inline::BookmarkKind::Start {
                            bookmark_start_count += 1;
                        } else {
                            bookmark_end_count += 1;
                        }
                    }
                }
            }
            Block::StyledPara(p) => {
                let text = flatten_inlines(&p.inlines);
                if text.contains("Executive Summary") {
                    has_exec_summary = true;
                }
                if text.contains("Goals & Non-Goals") || text.contains("Goals") {
                    has_goals_nongoals = true;
                }
                for inline in &p.inlines {
                    if let Inline::Bookmark(kind, _name) = inline {
                        if *kind == loki_doc_model::content::inline::BookmarkKind::Start {
                            bookmark_start_count += 1;
                        } else {
                            bookmark_end_count += 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    println!(
        "exec_summary: {}, goals_nongoals: {}",
        has_exec_summary, has_goals_nongoals
    );
    println!(
        "Bookmark starts: {}, Bookmark ends: {}",
        bookmark_start_count, bookmark_end_count
    );

    assert!(
        has_exec_summary,
        "Executive Summary heading should be parsed"
    );
    assert!(
        has_goals_nongoals,
        "Goals & Non-Goals heading should be parsed"
    );
    assert_eq!(
        bookmark_start_count, 43,
        "should have exactly 43 bookmark starts"
    );
    assert_eq!(
        bookmark_end_count, 43,
        "should have exactly 43 bookmark ends"
    );
}
