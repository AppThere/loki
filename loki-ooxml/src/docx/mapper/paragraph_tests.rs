// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for `paragraph`.

use super::*;
use crate::docx::import::DocxImportOptions;
use crate::docx::model::paragraph::{DocxPPr, DocxParaChild, DocxRun, DocxRunChild};
use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::style::catalog::{StyleCatalog, StyleId};
use loki_doc_model::style::para_style::ParagraphStyle;
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::ParaProps;
use loki_opc::PartData;
use std::collections::HashMap;

fn make_ctx<'a>(
    styles: &'a StyleCatalog,
    footnotes: &'a HashMap<i32, Vec<Block>>,
    endnotes: &'a HashMap<i32, Vec<Block>>,
    hyperlinks: &'a HashMap<String, String>,
    images: &'a HashMap<String, PartData>,
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

fn text_child(s: &str) -> DocxParaChild {
    DocxParaChild::Run(DocxRun {
        rpr: None,
        children: vec![DocxRunChild::Text {
            text: s.to_string(),
            preserve: false,
        }],
    })
}

fn default_opts() -> DocxImportOptions {
    DocxImportOptions::default()
}

fn empty_maps() -> (
    HashMap<i32, Vec<Block>>,
    HashMap<i32, Vec<Block>>,
    HashMap<String, String>,
    HashMap<String, PartData>,
) {
    (
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    )
}

#[test]
fn plain_paragraph_produces_styled_para() {
    let styles = StyleCatalog::default();
    let (fn_m, en_m, hl_m, img_m) = empty_maps();
    let opts = default_opts();
    let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

    let p = DocxParagraph {
        ppr: None,
        children: vec![text_child("hello world")],
    };
    let blocks = map_paragraph(&p, &mut ctx);
    assert_eq!(blocks.len(), 1);
    if let Block::StyledPara(sp) = &blocks[0] {
        assert_eq!(sp.style_id, None);
        assert_eq!(sp.inlines, vec![Inline::Str("hello world".into())]);
    } else {
        panic!("expected StyledPara");
    }
}

#[test]
fn paragraph_with_style_id() {
    let styles = StyleCatalog::default();
    let (fn_m, en_m, hl_m, img_m) = empty_maps();
    let opts = default_opts();
    let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

    let p = DocxParagraph {
        ppr: Some(DocxPPr {
            style_id: Some("BodyText".into()),
            ..Default::default()
        }),
        children: vec![text_child("text")],
    };
    let blocks = map_paragraph(&p, &mut ctx);
    if let Block::StyledPara(sp) = &blocks[0] {
        assert_eq!(sp.style_id, Some(StyleId::new("BodyText")));
    } else {
        panic!("expected StyledPara");
    }
}

#[test]
fn heading_via_direct_outline_level_emits_heading_block() {
    let styles = StyleCatalog::default();
    let (fn_m, en_m, hl_m, img_m) = empty_maps();
    let opts = DocxImportOptions {
        emit_heading_blocks: true,
        ..Default::default()
    };
    let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

    let p = DocxParagraph {
        ppr: Some(DocxPPr {
            outline_lvl: Some(0),
            ..Default::default()
        }), // 0 = Heading1 in OOXML (0-indexed)
        children: vec![text_child("Title")],
    };
    let blocks = map_paragraph(&p, &mut ctx);
    // Should produce [Heading(1, ...)]
    assert_eq!(blocks.len(), 1);
    assert!(matches!(&blocks[0], Block::Heading(1, _, _)));
}

#[test]
fn heading_via_style_outline_level() {
    let mut styles = StyleCatalog::default();
    let heading_style = ParagraphStyle {
        id: StyleId::new("Heading1"),
        display_name: Some("Heading 1".into()),
        parent: None,
        linked_char_style: None,
        next_style_id: None,
        para_props: ParaProps {
            outline_level: Some(1),
            ..Default::default()
        },
        char_props: CharProps::default(),
        is_default: false,
        is_custom: false,
        extensions: ExtensionBag::default(),
    };
    styles
        .paragraph_styles
        .insert(StyleId::new("Heading1"), heading_style);

    let (fn_m, en_m, hl_m, img_m) = empty_maps();
    let opts = DocxImportOptions {
        emit_heading_blocks: true,
        ..Default::default()
    };
    let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

    let p = DocxParagraph {
        ppr: Some(DocxPPr {
            style_id: Some("Heading1".into()),
            ..Default::default()
        }),
        children: vec![text_child("Chapter 1")],
    };
    let blocks = map_paragraph(&p, &mut ctx);
    assert_eq!(blocks.len(), 1);
    assert!(matches!(&blocks[0], Block::Heading(1, _, _)));
}

#[test]
fn heading_suppressed_when_option_disabled() {
    let styles = StyleCatalog::default();
    let (fn_m, en_m, hl_m, img_m) = empty_maps();
    let opts = DocxImportOptions {
        emit_heading_blocks: false,
        ..Default::default()
    };
    let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

    let p = DocxParagraph {
        ppr: Some(DocxPPr {
            outline_lvl: Some(0),
            ..Default::default()
        }),
        children: vec![text_child("No heading block")],
    };
    let blocks = map_paragraph(&p, &mut ctx);
    assert_eq!(blocks.len(), 1);
    assert!(matches!(&blocks[0], Block::StyledPara(_)));
}

#[test]
fn empty_paragraph_produces_empty_inlines() {
    let styles = StyleCatalog::default();
    let (fn_m, en_m, hl_m, img_m) = empty_maps();
    let opts = default_opts();
    let mut ctx = make_ctx(&styles, &fn_m, &en_m, &hl_m, &img_m, &opts);

    let p = DocxParagraph {
        ppr: None,
        children: vec![],
    };
    let blocks = map_paragraph(&p, &mut ctx);
    assert_eq!(blocks.len(), 1);
    if let Block::StyledPara(sp) = &blocks[0] {
        assert!(sp.inlines.is_empty());
    } else {
        panic!("expected StyledPara");
    }
}
