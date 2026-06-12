// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the OOXML field state machine and field instruction parser.

use super::field_state::parse_field_instruction;
use super::map_inlines;
use crate::docx::import::DocxImportOptions;
use crate::docx::model::paragraph::{DocxParaChild, DocxRun, DocxRunChild};
use loki_doc_model::content::block::Block;
use loki_doc_model::content::field::types::{CrossRefFormat, FieldKind};
use loki_doc_model::content::inline::Inline;
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
fn page_field_assembled() {
    let (styles, fn_m, en_m, hl_m, img_m, opts) = default_ctx();
    let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
    let children = vec![
        DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::FldChar {
                fld_char_type: "begin".into(),
            }],
        }),
        DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::InstrText {
                text: " PAGE ".into(),
            }],
        }),
        DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::FldChar {
                fld_char_type: "separate".into(),
            }],
        }),
        DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::Text {
                text: "42".into(),
                preserve: false,
            }],
        }),
        DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::FldChar {
                fld_char_type: "end".into(),
            }],
        }),
    ];
    let inlines = map_inlines(&children, &mut ctx);
    assert_eq!(inlines.len(), 1);
    if let Inline::Field(f) = &inlines[0] {
        assert_eq!(f.kind, FieldKind::PageNumber);
        assert_eq!(f.current_value.as_deref(), Some("42"));
    } else {
        panic!("expected Field, got {:?}", inlines[0]);
    }
}

#[test]
fn field_without_separate_has_no_current_value() {
    let (styles, fn_m, en_m, hl_m, img_m, opts) = default_ctx();
    let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
    let children = vec![
        DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::FldChar {
                fld_char_type: "begin".into(),
            }],
        }),
        DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::InstrText {
                text: "TITLE".into(),
            }],
        }),
        DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::FldChar {
                fld_char_type: "end".into(),
            }],
        }),
    ];
    let inlines = map_inlines(&children, &mut ctx);
    if let Inline::Field(f) = &inlines[0] {
        assert_eq!(f.kind, FieldKind::Title);
        assert!(f.current_value.is_none());
    } else {
        panic!("expected Field");
    }
}

#[test]
fn nested_fields_do_not_panic() {
    let (styles, fn_m, en_m, hl_m, img_m, opts) = default_ctx();
    let mut ctx = make_ctx(&fn_m, &en_m, &hl_m, &img_m, &styles, &opts);
    // Outer: IF { inner: DATE }
    let children = vec![
        DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::FldChar {
                fld_char_type: "begin".into(),
            }],
        }),
        DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::InstrText {
                text: " IF ".into(),
            }],
        }),
        // Inner field begin (depth 2)
        DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::FldChar {
                fld_char_type: "begin".into(),
            }],
        }),
        DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::InstrText {
                text: " DATE ".into(),
            }],
        }),
        DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::FldChar {
                fld_char_type: "end".into(),
            }],
        }),
        // Outer field end
        DocxParaChild::Run(DocxRun {
            rpr: None,
            children: vec![DocxRunChild::FldChar {
                fld_char_type: "end".into(),
            }],
        }),
    ];
    let inlines = map_inlines(&children, &mut ctx);
    // Outer IF field should be assembled (as Raw since we don't know IF)
    assert_eq!(inlines.len(), 1);
    assert!(matches!(&inlines[0], Inline::Field(_)));
}

#[test]
fn parse_date_field_with_format_switch() {
    let kind = parse_field_instruction(r#" DATE \@ "MMMM d, yyyy" "#);
    assert!(matches!(kind, FieldKind::Date { format: Some(ref s) } if s == "MMMM d, yyyy"));
}

#[test]
fn parse_ref_field() {
    let kind = parse_field_instruction(" REF _MyBookmark ");
    assert!(
        matches!(kind, FieldKind::CrossReference { target, format: CrossRefFormat::Number } if target == "_MyBookmark")
    );
}

#[test]
fn parse_unknown_field_is_raw() {
    let kind = parse_field_instruction(" HYPERLINK \"https://example.com\" ");
    assert!(matches!(kind, FieldKind::Raw { instruction } if instruction.contains("HYPERLINK")));
}
