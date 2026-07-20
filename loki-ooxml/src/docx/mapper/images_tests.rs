// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the image / text-box drawing mapper (extracted for the ceiling).

use super::*;
use crate::docx::import::DocxImportOptions;
use loki_doc_model::style::catalog::StyleCatalog;
use loki_opc::PartData;
use std::collections::HashMap;

fn make_ctx<'a>(
    images: &'a HashMap<String, PartData>,
    styles: &'a StyleCatalog,
    footnotes: &'a HashMap<i32, Vec<loki_doc_model::content::block::Block>>,
    endnotes: &'a HashMap<i32, Vec<loki_doc_model::content::block::Block>>,
    hyperlinks: &'a HashMap<String, String>,
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

#[test]
fn missing_rel_id_returns_none_with_warning() {
    let images = HashMap::new();
    let catalog = StyleCatalog::default();
    let fn_map = HashMap::new();
    let en_map = HashMap::new();
    let hl_map = HashMap::new();
    let opts = DocxImportOptions::default();
    let mut ctx = make_ctx(&images, &catalog, &fn_map, &en_map, &hl_map, &opts);

    let drawing = DocxDrawing {
        rel_id: None,
        cx: None,
        cy: None,
        descr: None,
        name: None,
        is_anchor: false,
        wrap: None,
        txbx: Vec::new(),
        fill_color: None,
        line_color: None,
        line_w_emu: None,
    };
    let result = map_drawing(&drawing, &mut ctx);
    assert!(result.is_none());
    assert_eq!(ctx.warnings.len(), 1);
    assert!(matches!(
        &ctx.warnings[0],
        OoxmlWarning::UnresolvedImage { rel_id } if rel_id.is_empty()
    ));
}

#[test]
fn unresolved_rel_id_returns_none_with_warning() {
    let images = HashMap::new();
    let catalog = StyleCatalog::default();
    let fn_map = HashMap::new();
    let en_map = HashMap::new();
    let hl_map = HashMap::new();
    let opts = DocxImportOptions::default();
    let mut ctx = make_ctx(&images, &catalog, &fn_map, &en_map, &hl_map, &opts);

    let drawing = DocxDrawing {
        rel_id: Some("rId99".into()),
        cx: None,
        cy: None,
        descr: None,
        name: None,
        is_anchor: false,
        wrap: None,
        txbx: Vec::new(),
        fill_color: None,
        line_color: None,
        line_w_emu: None,
    };
    let result = map_drawing(&drawing, &mut ctx);
    assert!(result.is_none());
    assert!(matches!(
        &ctx.warnings[0],
        OoxmlWarning::UnresolvedImage { rel_id } if rel_id == "rId99"
    ));
}

#[test]
fn resolved_image_no_embed_returns_rel_id_as_url() {
    let mut images = HashMap::new();
    images.insert("rId1".into(), PartData::new(vec![0u8, 1, 2], "image/png"));

    let catalog = StyleCatalog::default();
    let fn_map = HashMap::new();
    let en_map = HashMap::new();
    let hl_map = HashMap::new();
    let opts = DocxImportOptions {
        embed_images: false,
        ..Default::default()
    };
    let mut ctx = make_ctx(&images, &catalog, &fn_map, &en_map, &hl_map, &opts);

    let drawing = DocxDrawing {
        rel_id: Some("rId1".into()),
        cx: Some(914_400),
        cy: Some(685_800),
        descr: Some("A test image".into()),
        name: Some("img1".into()),
        is_anchor: false,
        wrap: None,
        txbx: Vec::new(),
        fill_color: None,
        line_color: None,
        line_w_emu: None,
    };
    let result = map_drawing(&drawing, &mut ctx).unwrap();
    if let Inline::Image(attr, alt, target) = result {
        assert_eq!(target.url, "rId1");
        assert_eq!(target.title.as_deref(), Some("img1"));
        assert!(matches!(&alt[..], [Inline::Str(s)] if s == "A test image"));
        assert!(attr.kv.iter().any(|(k, v)| k == "cx_emu" && v == "914400"));
        assert!(attr.kv.iter().any(|(k, v)| k == "cy_emu" && v == "685800"));
        assert!(!attr.classes.contains(&"floating".to_string()));
    } else {
        panic!("expected Image inline");
    }
}

#[test]
fn anchor_drawing_gets_floating_class() {
    let mut images = HashMap::new();
    images.insert("rId2".into(), PartData::new(vec![], "image/jpeg"));

    let catalog = StyleCatalog::default();
    let fn_map = HashMap::new();
    let en_map = HashMap::new();
    let hl_map = HashMap::new();
    let opts = DocxImportOptions {
        embed_images: false,
        ..Default::default()
    };
    let mut ctx = make_ctx(&images, &catalog, &fn_map, &en_map, &hl_map, &opts);

    let drawing = DocxDrawing {
        rel_id: Some("rId2".into()),
        cx: None,
        cy: None,
        descr: None,
        name: None,
        is_anchor: true,
        wrap: None,
        txbx: Vec::new(),
        fill_color: None,
        line_color: None,
        line_w_emu: None,
    };
    let result = map_drawing(&drawing, &mut ctx).unwrap();
    if let Inline::Image(attr, _, _) = result {
        assert!(attr.classes.contains(&"floating".to_string()));
    } else {
        panic!("expected Image");
    }
}

#[test]
fn anchor_drawing_carries_wrap_mode() {
    use loki_doc_model::content::float::{FloatWrap, TextWrap, WrapSide};

    let mut images = HashMap::new();
    images.insert("rId3".into(), PartData::new(vec![], "image/png"));

    let catalog = StyleCatalog::default();
    let fn_map = HashMap::new();
    let en_map = HashMap::new();
    let hl_map = HashMap::new();
    let opts = DocxImportOptions {
        embed_images: false,
        ..Default::default()
    };
    let mut ctx = make_ctx(&images, &catalog, &fn_map, &en_map, &hl_map, &opts);

    let drawing = DocxDrawing {
        rel_id: Some("rId3".into()),
        cx: None,
        cy: None,
        descr: None,
        name: None,
        is_anchor: true,
        wrap: Some(FloatWrap {
            wrap: TextWrap::Tight,
            side: WrapSide::Left,
            behind_text: false,
        }),
        txbx: Vec::new(),
        fill_color: None,
        line_color: None,
        line_w_emu: None,
    };
    let result = map_drawing(&drawing, &mut ctx).unwrap();
    if let Inline::Image(attr, _, _) = result {
        assert!(attr.classes.contains(&FLOATING_CLASS.to_string()));
        assert_eq!(
            FloatWrap::read(&attr),
            Some(FloatWrap {
                wrap: TextWrap::Tight,
                side: WrapSide::Left,
                behind_text: false,
            })
        );
    } else {
        panic!("expected Image");
    }
}

#[test]
fn text_box_drawing_maps_to_inline_text_box() {
    use crate::docx::model::paragraph::DocxParagraph;
    let images = std::collections::HashMap::new();
    let styles = StyleCatalog::default();
    let fns = std::collections::HashMap::new();
    let ens = std::collections::HashMap::new();
    let links = std::collections::HashMap::new();
    let opts = DocxImportOptions::default();
    let mut ctx = make_ctx(&images, &styles, &fns, &ens, &links, &opts);
    let drawing = DocxDrawing {
        cx: Some(1_828_800),
        cy: Some(731_520),
        is_anchor: true,
        txbx: vec![DocxParagraph::default()],
        fill_color: Some("FDF0E6".into()),
        line_color: Some("ED7D31".into()),
        ..Default::default()
    };
    let result = map_drawing(&drawing, &mut ctx).expect("maps");
    match result {
        Inline::TextBox(attr, _blocks) => {
            assert!(
                attr.kv
                    .iter()
                    .any(|(k, v)| k == "textbox-fill" && v == "FDF0E6")
            );
            assert!(
                attr.kv
                    .iter()
                    .any(|(k, v)| k == "textbox-line" && v == "ED7D31")
            );
            assert!(attr.kv.iter().any(|(k, v)| k == "cx_emu" && v == "1828800"));
        }
        other => panic!("expected Inline::TextBox, got {other:?}"),
    }
}
