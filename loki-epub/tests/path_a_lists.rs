// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! §10 path-A lists in EPUB output: `StyledPara.list_id` runs render as
//! nested `<ul>`/`<ol>` markup, well-formed, with `<ol>` chosen from the
//! referenced style's level-0 kind.

use std::io::{Cursor, Read};

use loki_doc_model::Document;
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::io::DocumentExport;
use loki_doc_model::style::list_defaults::{
    default_bullet_list_style, default_numbered_list_style,
};
use loki_doc_model::style::props::para_props::ParaProps;
use loki_epub::{EpubExport, EpubOptions};
use quick_xml::Reader;
use quick_xml::events::Event;
use zip::ZipArchive;

fn item(text: &str, list_id: &str, level: u8) -> Block {
    Block::StyledPara(StyledParagraph {
        style_id: None,
        direct_para_props: Some(Box::new(ParaProps {
            list_id: Some(loki_doc_model::style::list_style::ListId::new(list_id)),
            list_level: Some(level),
            ..Default::default()
        })),
        direct_char_props: None,
        inlines: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    })
}

fn content_xhtml(doc: &Document) -> String {
    let mut buf = Cursor::new(Vec::new());
    EpubExport::export(doc, &mut buf, EpubOptions::default()).expect("export");
    let mut archive = ZipArchive::new(Cursor::new(buf.into_inner())).expect("zip");
    let name = (0..archive.len())
        .map(|i| archive.by_index(i).map(|f| f.name().to_string()))
        .filter_map(Result::ok)
        .find(|n| n.ends_with(".xhtml") && !n.contains("nav"))
        .expect("content document");
    let mut s = String::new();
    archive
        .by_name(&name)
        .expect("entry")
        .read_to_string(&mut s)
        .expect("utf-8");
    s
}

/// Walks the XML to prove well-formedness (mismatched `<li>` nesting would
/// fail the parse).
fn assert_well_formed(xml: &str) {
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => panic!("content not well-formed: {e}"),
        }
    }
}

#[test]
fn nested_bullet_run_renders_as_nested_ul() {
    let bullet = default_bullet_list_style();
    let mut doc = Document::new();
    doc.styles
        .list_styles
        .insert(bullet.id.clone(), bullet.clone());
    doc.sections[0].blocks = vec![
        item("top", bullet.id.as_str(), 0),
        item("nested", bullet.id.as_str(), 1),
        item("top again", bullet.id.as_str(), 0),
        Block::Para(vec![Inline::Str("after".into())]),
    ];

    let xhtml = content_xhtml(&doc);
    assert_well_formed(&xhtml);
    // The nested item opens a sub-list inside the first item's <li>.
    assert!(
        xhtml.contains("<li>top<ul>"),
        "nested list must open inside the parent li; got: {xhtml}"
    );
    assert!(xhtml.contains("<li>nested</li>"), "{xhtml}");
    // The run closes before the following paragraph.
    let ul_close = xhtml.rfind("</ul>").expect("closing ul");
    let after = xhtml.find("<p>after</p>").expect("following para");
    assert!(ul_close < after, "list must close before the paragraph");
    assert_eq!(
        xhtml.matches("<ul>").count(),
        xhtml.matches("</ul>").count()
    );
}

#[test]
fn numbered_style_renders_as_ol_and_unknown_id_as_ul() {
    let numbered = default_numbered_list_style();
    let mut doc = Document::new();
    doc.styles
        .list_styles
        .insert(numbered.id.clone(), numbered.clone());
    doc.sections[0].blocks = vec![
        item("first", numbered.id.as_str(), 0),
        item("second", numbered.id.as_str(), 0),
        // An id the catalog does not define: bullet fallback.
        item("orphan", "no-such-style", 0),
    ];

    let xhtml = content_xhtml(&doc);
    assert_well_formed(&xhtml);
    assert!(xhtml.contains("<ol>"), "numbered style must render <ol>");
    assert!(xhtml.contains("<li>first</li>"), "{xhtml}");
    assert!(
        xhtml.contains("<ul>\n<li>orphan</li>"),
        "unknown id must fall back to a bullet list; got: {xhtml}"
    );
}
