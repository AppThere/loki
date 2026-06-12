// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for run, hyperlink, bookmark, and note mapping.

use super::map_inlines;
use crate::docx::import::DocxImportOptions;
use crate::docx::model::paragraph::{DocxHyperlink, DocxParaChild, DocxRPr, DocxRun, DocxRunChild};
use crate::error::{NoteKind as WarnNoteKind, OoxmlWarning};
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{BookmarkKind, Inline, NoteKind};
use loki_doc_model::style::catalog::StyleCatalog;
use std::collections::HashMap;

use super::super::document::MappingContext;

fn make_ctx<'a>(
    footnotes: &'a HashMap<i32, Vec<Block>>,
    endnotes: &'a HashMap<i32, Vec<Block>>,
    hyperlinks: &'a HashMap<String, String>,
    images: &'a HashMap<String, loki_opc::PartData>,
    styles: &'a StyleCatalog,
    options: &'a DocxImportOptions,
) -> MappingContext<'a> {
    MappingContext {
        styles,
        footnotes,
        endnotes,
        hyperlinks,
        images,
        options,
        warnings: Vec::new(),
        open_bookmarks: Vec::new(),
    }
}

fn plain_run(text: &str) -> DocxParaChild {
    DocxParaChild::Run(DocxRun {
        rpr: None,
        children: vec![DocxRunChild::Text {
            text: text.to_string(),
            preserve: false,
        }],
    })
}

fn bold_run(text: &str) -> DocxParaChild {
    DocxParaChild::Run(DocxRun {
        rpr: Some(DocxRPr {
            bold: Some(true),
            ..Default::default()
        }),
        children: vec![DocxRunChild::Text {
            text: text.to_string(),
            preserve: false,
        }],
    })
}

fn default_ctx() -> (
    StyleCatalog,
    HashMap<i32, Vec<Block>>,
    HashMap<i32, Vec<Block>>,
    HashMap<String, String>,
    HashMap<String, loki_opc::PartData>,
    DocxImportOptions,
) {
    (
        StyleCatalog::default(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        DocxImportOptions::default(),
    )
}

#[test]
fn plain_text_run() {
    let (styles, fn_m, en_m, hl_m, img_m, opts) = default_ctx();
    let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
    let children = vec![plain_run("hello")];
    let inlines = map_inlines(&children, &mut ctx);
    assert_eq!(inlines, vec![Inline::Str("hello".into())]);
}

#[test]
fn bold_run_produces_styled_run() {
    let (styles, fn_m, en_m, hl_m, img_m, opts) = default_ctx();
    let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
    let children = vec![bold_run("bold text")];
    let inlines = map_inlines(&children, &mut ctx);
    assert_eq!(inlines.len(), 1);
    if let Inline::StyledRun(sr) = &inlines[0] {
        assert_eq!(sr.direct_props.as_ref().unwrap().bold, Some(true));
        assert_eq!(sr.content, vec![Inline::Str("bold text".into())]);
    } else {
        panic!("expected StyledRun, got {:?}", inlines[0]);
    }
}

#[test]
fn hyperlink_with_url() {
    let (styles, fn_m, en_m, mut hl_m, img_m, opts) = default_ctx();
    hl_m.insert("rId1".into(), "https://example.com".into());
    let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
    let children = vec![DocxParaChild::Hyperlink(DocxHyperlink {
        rel_id: Some("rId1".into()),
        anchor: None,
        runs: vec![DocxRun {
            rpr: None,
            children: vec![DocxRunChild::Text {
                text: "click".into(),
                preserve: false,
            }],
        }],
    })];
    let inlines = map_inlines(&children, &mut ctx);
    assert_eq!(inlines.len(), 1);
    if let Inline::Link(_, content, target) = &inlines[0] {
        assert_eq!(target.url, "https://example.com");
        assert_eq!(content, &vec![Inline::Str("click".into())]);
    } else {
        panic!("expected Link");
    }
}

#[test]
fn footnote_ref_with_content() {
    let (styles, mut fn_m, en_m, hl_m, img_m, opts) = default_ctx();
    fn_m.insert(1, vec![Block::Para(vec![Inline::Str("note text".into())])]);
    let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
    let children = vec![DocxParaChild::Run(DocxRun {
        rpr: None,
        children: vec![DocxRunChild::FootnoteRef { id: 1 }],
    })];
    let inlines = map_inlines(&children, &mut ctx);
    assert!(
        matches!(&inlines[0], Inline::Note(NoteKind::Footnote, blocks) if !blocks.is_empty())
    );
}

#[test]
fn footnote_ref_missing_emits_warning() {
    let (styles, fn_m, en_m, hl_m, img_m, opts) = default_ctx();
    let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
    let children = vec![DocxParaChild::Run(DocxRun {
        rpr: None,
        children: vec![DocxRunChild::FootnoteRef { id: 99 }],
    })];
    let inlines = map_inlines(&children, &mut ctx);
    assert!(
        matches!(&inlines[0], Inline::Note(NoteKind::Footnote, blocks) if blocks.is_empty())
    );
    assert_eq!(ctx.warnings.len(), 1);
    assert!(matches!(
        &ctx.warnings[0],
        OoxmlWarning::MissingNoteContent {
            id: 99,
            kind: WarnNoteKind::Footnote
        }
    ));
}

#[test]
fn bookmark_start_and_end() {
    let (styles, fn_m, en_m, hl_m, img_m, opts) = default_ctx();
    let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
    let children = vec![
        DocxParaChild::BookmarkStart {
            id: "1".into(),
            name: "myBookmark".into(),
        },
        plain_run("text"),
        DocxParaChild::BookmarkEnd { id: "1".into() },
    ];
    let inlines = map_inlines(&children, &mut ctx);
    assert_eq!(inlines.len(), 3);
    assert!(
        matches!(&inlines[0], Inline::Bookmark(BookmarkKind::Start, name) if name == "myBookmark")
    );
    assert!(
        matches!(&inlines[2], Inline::Bookmark(BookmarkKind::End, name) if name == "myBookmark")
    );
}
