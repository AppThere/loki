// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2025 AppThere contributors

//! Tests for the document reader submodules.

use super::*;
use quick_xml::Reader;
use quick_xml::events::Event;

use crate::odt::model::document::{OdfBodyChild, OdfListItemChild};
use crate::odt::model::fields::OdfField;
use crate::odt::model::frames::OdfFrameKind;
use crate::odt::model::notes::OdfNoteClass;
use crate::odt::model::paragraph::{OdfParagraph, OdfParagraphChild};
use crate::version::OdfVersion;

// ── Test helper ───────────────────────────────────────────────────────────────

/// Parse the first `text:p` or `text:h` found in `xml` and return the
/// resulting [`OdfParagraph`].
fn parse_first_para(xml: &[u8]) -> OdfParagraph {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf).expect("xml error") {
            Event::Start(ref e) => {
                let local = e.local_name().into_inner();
                if local == b"p" || local == b"h" {
                    return read_paragraph(&mut reader, e).expect("read_paragraph failed");
                }
            }
            Event::Eof => panic!("no text:p / text:h found in test XML"),
            _ => {}
        }
    }
}

// ── Test cases ────────────────────────────────────────────────────────────────

#[test]
fn plain_text_paragraph() {
    let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <text:p text:style-name="Body_20_Text">Hello, World!</text:p>
</root>"#;
    let para = parse_first_para(xml);
    assert_eq!(para.style_name.as_deref(), Some("Body_20_Text"));
    assert!(!para.is_heading);
    assert_eq!(para.outline_level, None);
    assert_eq!(para.children.len(), 1);
    match &para.children[0] {
        OdfParagraphChild::Text(s) => assert_eq!(s, "Hello, World!"),
        other => panic!("expected Text, got {:?}", other),
    }
}

#[test]
fn paragraph_with_span() {
    let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <text:p>Hello <text:span text:style-name="Bold">World</text:span>!</text:p>
</root>"#;
    let para = parse_first_para(xml);
    assert_eq!(para.children.len(), 3);
    match &para.children[1] {
        OdfParagraphChild::Span(span) => {
            assert_eq!(span.style_name.as_deref(), Some("Bold"));
            assert_eq!(span.children.len(), 1);
            match &span.children[0] {
                OdfParagraphChild::Text(s) => assert_eq!(s, "World"),
                other => panic!("expected Text in span, got {:?}", other),
            }
        }
        other => panic!("expected Span, got {:?}", other),
    }
}

#[test]
fn heading_with_outline_level() {
    let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <text:h text:style-name="Heading_20_2" text:outline-level="2">Section 1.1</text:h>
</root>"#;
    let para = parse_first_para(xml);
    assert!(para.is_heading);
    assert_eq!(para.outline_level, Some(2));
    assert_eq!(para.style_name.as_deref(), Some("Heading_20_2"));
    assert_eq!(para.children.len(), 1);
    match &para.children[0] {
        OdfParagraphChild::Text(s) => assert_eq!(s, "Section 1.1"),
        other => panic!("expected Text, got {:?}", other),
    }
}

#[test]
fn paragraph_with_footnote() {
    let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <text:p>See note<text:note text:id="ftn1" text:note-class="footnote"><text:note-citation>1</text:note-citation><text:note-body><text:p text:style-name="Footnote">Footnote text.</text:p></text:note-body></text:note>.</text:p>
</root>"#;
    let para = parse_first_para(xml);
    let note = para
        .children
        .iter()
        .find_map(|c| match c {
            OdfParagraphChild::Note(n) => Some(n),
            _ => None,
        })
        .expect("no Note child");
    assert_eq!(note.id.as_deref(), Some("ftn1"));
    assert_eq!(note.note_class, OdfNoteClass::Footnote);
    assert_eq!(note.citation.as_deref(), Some("1"));
    assert_eq!(note.body.len(), 1);
    assert_eq!(note.body[0].style_name.as_deref(), Some("Footnote"));
    match &note.body[0].children[0] {
        OdfParagraphChild::Text(s) => assert_eq!(s, "Footnote text."),
        other => panic!("expected Text in footnote body, got {:?}", other),
    }
}

#[test]
fn page_number_field() {
    let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <text:p><text:page-number text:select-page="current">1</text:page-number></text:p>
</root>"#;
    let para = parse_first_para(xml);
    assert_eq!(para.children.len(), 1);
    match &para.children[0] {
        OdfParagraphChild::Field(OdfField::PageNumber { select_page }) => {
            assert_eq!(select_page.as_deref(), Some("current"));
        }
        other => panic!("expected PageNumber field, got {:?}", other),
    }
}

#[test]
fn paragraph_with_hyperlink() {
    let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
      xmlns:xlink="http://www.w3.org/1999/xlink">
  <text:p><text:a xlink:href="https://example.com" text:style-name="Internet_20_Link">Click here</text:a></text:p>
</root>"#;
    let para = parse_first_para(xml);
    assert_eq!(para.children.len(), 1);
    match &para.children[0] {
        OdfParagraphChild::Hyperlink(link) => {
            assert_eq!(link.href.as_deref(), Some("https://example.com"));
            assert_eq!(link.style_name.as_deref(), Some("Internet_20_Link"));
            assert_eq!(link.children.len(), 1);
            match &link.children[0] {
                OdfParagraphChild::Text(s) => {
                    assert_eq!(s, "Click here")
                }
                other => {
                    panic!("expected Text in link, got {:?}", other)
                }
            }
        }
        other => panic!("expected Hyperlink, got {:?}", other),
    }
}

#[test]
fn paragraph_with_inline_image() {
    let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
      xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0"
      xmlns:xlink="http://www.w3.org/1999/xlink"
      xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0">
  <text:p>
    <draw:frame draw:name="Image1" text:anchor-type="as-char" svg:width="5cm" svg:height="3cm">
      <draw:image xlink:href="Pictures/img.png">
        <svg:title>Alt text</svg:title>
      </draw:image>
    </draw:frame>
  </text:p>
</root>"#;
    let para = parse_first_para(xml);
    let frame = para
        .children
        .iter()
        .find_map(|c| match c {
            OdfParagraphChild::Frame(f) => Some(f),
            _ => None,
        })
        .expect("no Frame child");
    assert_eq!(frame.name.as_deref(), Some("Image1"));
    assert_eq!(frame.anchor_type.as_deref(), Some("as-char"));
    assert_eq!(frame.width.as_deref(), Some("5cm"));
    assert_eq!(frame.height.as_deref(), Some("3cm"));
    match &frame.kind {
        OdfFrameKind::Image { href, title, .. } => {
            assert_eq!(href, "Pictures/img.png");
            assert_eq!(title.as_deref(), Some("Alt text"));
        }
        other => panic!("expected Image kind, got {:?}", other),
    }
}

#[test]
fn space_and_tab_elements() {
    let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <text:p>A<text:s text:c="3"/>B<text:tab/>C</text:p>
</root>"#;
    let para = parse_first_para(xml);
    assert_eq!(para.children.len(), 5);
    assert!(matches!(&para.children[0], OdfParagraphChild::Text(s) if s == "A"));
    assert!(matches!(
        &para.children[1],
        OdfParagraphChild::Space { count: 3 }
    ));
    assert!(matches!(&para.children[2], OdfParagraphChild::Text(s) if s == "B"));
    assert!(matches!(&para.children[3], OdfParagraphChild::Tab));
    assert!(matches!(&para.children[4], OdfParagraphChild::Text(s) if s == "C"));
}

#[test]
fn nested_spans() {
    let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <text:p><text:span text:style-name="Outer"><text:span text:style-name="Inner">deep</text:span></text:span></text:p>
</root>"#;
    let para = parse_first_para(xml);
    assert_eq!(para.children.len(), 1);
    let outer = match &para.children[0] {
        OdfParagraphChild::Span(s) => s,
        other => panic!("expected outer Span, got {:?}", other),
    };
    assert_eq!(outer.style_name.as_deref(), Some("Outer"));
    assert_eq!(outer.children.len(), 1);
    let inner = match &outer.children[0] {
        OdfParagraphChild::Span(s) => s,
        other => panic!("expected inner Span, got {:?}", other),
    };
    assert_eq!(inner.style_name.as_deref(), Some("Inner"));
    assert_eq!(inner.children.len(), 1);
    match &inner.children[0] {
        OdfParagraphChild::Text(t) => assert_eq!(t, "deep"),
        other => panic!("expected Text in inner span, got {:?}", other),
    }
}

// ── Body-level tests ──────────────────────────────────────────────────────────

#[test]
fn table_2x2_with_covered_cell() {
    let xml = br#"<?xml version="1.0"?>
<root xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0"
      xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <table:table table:name="T1">
    <table:table-column/>
    <table:table-column/>
    <table:table-row>
      <table:table-cell table:number-columns-spanned="1">
        <text:p>Cell A1</text:p>
      </table:table-cell>
      <table:table-cell>
        <text:p>Cell A2</text:p>
      </table:table-cell>
    </table:table-row>
    <table:table-row>
      <table:table-cell>
        <text:p>Cell B1</text:p>
      </table:table-cell>
      <table:covered-table-cell/>
    </table:table-row>
  </table:table>
</root>"#;
    let mut reader = Reader::from_reader(xml.as_ref());
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let table = loop {
        buf.clear();
        match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(ref e) if e.local_name().into_inner() == b"table" => {
                break read_table(&mut reader, e).unwrap();
            }
            Event::Eof => panic!("no table found"),
            _ => {}
        }
    };
    assert_eq!(table.name.as_deref(), Some("T1"));
    assert_eq!(table.col_defs.len(), 2);
    assert_eq!(table.rows.len(), 2);

    let row0 = &table.rows[0];
    assert_eq!(row0.cells.len(), 2);
    assert!(!row0.cells[0].is_covered);
    assert_eq!(row0.cells[0].col_span, 1);
    match &row0.cells[0].paragraphs[0].children[0] {
        OdfParagraphChild::Text(s) => assert_eq!(s, "Cell A1"),
        other => panic!("{:?}", other),
    }

    let row1 = &table.rows[1];
    assert_eq!(row1.cells.len(), 2);
    assert!(!row1.cells[0].is_covered);
    assert!(row1.cells[1].is_covered);
}

#[test]
fn list_with_nesting() {
    let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <text:list text:style-name="List1">
    <text:list-item>
      <text:p>Item 1</text:p>
      <text:list>
        <text:list-item>
          <text:p>Item 1.1</text:p>
        </text:list-item>
      </text:list>
    </text:list-item>
  </text:list>
</root>"#;
    let mut reader = Reader::from_reader(xml.as_ref());
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let list = loop {
        buf.clear();
        match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(ref e) if e.local_name().into_inner() == b"list" => {
                break read_list(&mut reader, e, None, 0).unwrap();
            }
            Event::Eof => panic!("no list found"),
            _ => {}
        }
    };
    assert_eq!(list.style_name.as_deref(), Some("List1"));
    assert_eq!(list.items.len(), 1);
    let item = &list.items[0];
    // children: Paragraph("Item 1"), List(nested)
    assert_eq!(item.children.len(), 2);
    match &item.children[0] {
        OdfListItemChild::Paragraph(p) => {
            assert_eq!(p.list_context.as_ref().unwrap().level, 0);
            match &p.children[0] {
                OdfParagraphChild::Text(s) => assert_eq!(s, "Item 1"),
                other => panic!("{:?}", other),
            }
        }
        other => panic!("expected Paragraph, got {:?}", other),
    }
    match &item.children[1] {
        OdfListItemChild::List(nested) => {
            assert_eq!(nested.items.len(), 1);
            match &nested.items[0].children[0] {
                OdfListItemChild::Paragraph(p) => {
                    assert_eq!(p.list_context.as_ref().unwrap().level, 1);
                }
                other => panic!("{:?}", other),
            }
        }
        other => panic!("expected nested List, got {:?}", other),
    }
}

#[test]
fn read_document_version_present() {
    let xml = br#"<?xml version="1.0"?>
<office:document-content
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
  office:version="1.2">
  <office:body>
    <office:text>
      <text:p text:style-name="Standard">Hello</text:p>
    </office:text>
  </office:body>
</office:document-content>"#;
    let doc = read_document(xml).unwrap();
    assert_eq!(doc.version, OdfVersion::V1_2);
    assert!(!doc.version_was_absent);
    assert_eq!(doc.body_children.len(), 1);
    assert!(matches!(
        &doc.body_children[0],
        OdfBodyChild::Paragraph(p) if p.style_name.as_deref() == Some("Standard")
    ));
}

#[test]
fn read_document_version_absent() {
    let xml = br#"<?xml version="1.0"?>
<office:document-content
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <office:body>
    <office:text>
      <text:p>No version</text:p>
    </office:text>
  </office:body>
</office:document-content>"#;
    let doc = read_document(xml).unwrap();
    assert_eq!(doc.version, OdfVersion::V1_1);
    assert!(doc.version_was_absent);
    assert_eq!(doc.body_children.len(), 1);
}

#[test]
fn toc_parsing() {
    let xml = br#"<?xml version="1.0"?>
<root xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
  <text:table-of-content text:name="TOC1">
    <text:table-of-content-source text:outline-level="2"/>
    <text:index-body>
      <text:p>Entry one</text:p>
      <text:p>Entry two</text:p>
    </text:index-body>
  </text:table-of-content>
</root>"#;
    let mut reader = Reader::from_reader(xml.as_ref());
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let toc = loop {
        buf.clear();
        match reader.read_event_into(&mut buf).unwrap() {
            Event::Start(ref e) if e.local_name().into_inner() == b"table-of-content" => {
                break read_toc(&mut reader, e).unwrap();
            }
            Event::Eof => panic!("no toc found"),
            _ => {}
        }
    };
    assert_eq!(toc.name.as_deref(), Some("TOC1"));
    assert_eq!(toc.source_outline_level, 2);
    assert_eq!(toc.body_paragraphs.len(), 2);
    match &toc.body_paragraphs[0].children[0] {
        OdfParagraphChild::Text(s) => assert_eq!(s, "Entry one"),
        other => panic!("{:?}", other),
    }
}
