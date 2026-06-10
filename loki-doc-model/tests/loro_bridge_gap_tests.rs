// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Loro bridge round-trip tests for L-severity gap fixes.
//!
//! Verifies that language, border, padding, page_break_before, orphan_control,
//! and outline_level all survive a document_to_loro → loro_to_document cycle.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::{Inline, StyledRun};
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::meta::language::LanguageTag;
use loki_doc_model::style::props::border::{Border, BorderStyle};
use loki_doc_model::style::props::char_props::CharProps;
use loki_doc_model::style::props::para_props::ParaProps;
use loki_primitives::color::DocumentColor;
use loki_primitives::units::Points;

fn round_trip(doc: &Document) -> Document {
    let loro = document_to_loro(doc).expect("document_to_loro must succeed");
    loro_to_document(&loro).expect("loro_to_document must succeed")
}

fn single_block_doc(block: Block) -> Document {
    let mut doc = Document::new();
    doc.sections[0].blocks.push(block);
    doc
}

fn styled_para_with_char(props: CharProps) -> Block {
    let run = StyledRun {
        style_id: None,
        direct_props: Some(Box::new(props)),
        content: vec![Inline::Str("text".into())],
        attr: NodeAttr::default(),
    };
    Block::StyledPara(loki_doc_model::content::block::StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![Inline::StyledRun(run)],
        attr: NodeAttr::default(),
    })
}

// ── bridge_language_roundtrip ─────────────────────────────────────────────────

/// `CharProps.language`, `language_complex`, and `language_east_asian` must all
/// survive a Loro CRDT round-trip via text marks.
#[test]
fn bridge_language_roundtrip() {
    let mut props = CharProps::default();
    props.language = Some(LanguageTag::new("en-GB".to_string()));
    props.language_complex = Some(LanguageTag::new("ar-SA".to_string()));
    props.language_east_asian = Some(LanguageTag::new("ja-JP".to_string()));

    let doc = single_block_doc(styled_para_with_char(props));
    let recovered = round_trip(&doc);

    let run_props = recovered.sections[0].blocks.iter().find_map(|b| {
        if let Block::StyledPara(p) = b {
            p.inlines.iter().find_map(|i| {
                if let Inline::StyledRun(sr) = i {
                    sr.direct_props.as_deref()
                } else {
                    None
                }
            })
        } else {
            None
        }
    });

    let rp = run_props.expect("StyledRun with direct_props must survive round-trip");

    assert_eq!(
        rp.language.as_ref().map(|t| t.as_str()),
        Some("en-GB"),
        "language must survive Loro round-trip"
    );
    assert_eq!(
        rp.language_complex.as_ref().map(|t| t.as_str()),
        Some("ar-SA"),
        "language_complex must survive Loro round-trip"
    );
    assert_eq!(
        rp.language_east_asian.as_ref().map(|t| t.as_str()),
        Some("ja-JP"),
        "language_east_asian must survive Loro round-trip"
    );
}

// ── bridge_border_roundtrip ───────────────────────────────────────────────────

/// `ParaProps.border_top` must survive a Loro CRDT round-trip.
#[test]
fn bridge_border_roundtrip() {
    let mut para_props = ParaProps::default();
    para_props.border_top = Some(Border {
        style: BorderStyle::Solid,
        width: Points::new(1.0),
        color: Some(DocumentColor::from_hex("#FF0000").unwrap()),
        spacing: None,
    });
    para_props.border_bottom = Some(Border {
        style: BorderStyle::Dashed,
        width: Points::new(0.5),
        color: None,
        spacing: Some(Points::new(2.0)),
    });

    let block = Block::StyledPara(loki_doc_model::content::block::StyledParagraph {
        style_id: None,
        direct_para_props: Some(Box::new(para_props)),
        direct_char_props: None,
        inlines: vec![Inline::Str("bordered".into())],
        attr: NodeAttr::default(),
    });

    let doc = single_block_doc(block);
    let recovered = round_trip(&doc);

    let para = recovered.sections[0]
        .blocks
        .iter()
        .find_map(|b| {
            if let Block::StyledPara(p) = b {
                Some(p)
            } else {
                None
            }
        })
        .expect("StyledPara must survive round-trip");

    let pp = para
        .direct_para_props
        .as_deref()
        .expect("direct_para_props must be Some");

    let top = pp
        .border_top
        .as_ref()
        .expect("border_top must survive round-trip");
    assert_eq!(top.style, BorderStyle::Solid);
    assert!(
        (top.width.value() - 1.0).abs() < 0.001,
        "border_top width must be 1.0, got {}",
        top.width.value()
    );
    assert_eq!(
        top.color.as_ref().and_then(|c| c.to_hex()).as_deref(),
        Some("#FF0000"),
        "border_top color must round-trip"
    );

    let bottom = pp
        .border_bottom
        .as_ref()
        .expect("border_bottom must survive round-trip");
    assert_eq!(bottom.style, BorderStyle::Dashed);
    assert!(
        (bottom.spacing.map_or(0.0, |s| s.value()) - 2.0).abs() < 0.001,
        "border_bottom spacing must be 2.0"
    );
}

// ── bridge_para_fields_roundtrip ──────────────────────────────────────────────

/// `ParaProps.page_break_before`, `orphan_control`, `outline_level`, and
/// `padding_top/bottom/left/right` must survive a Loro CRDT round-trip.
#[test]
fn bridge_para_fields_roundtrip() {
    let mut para_props = ParaProps::default();
    para_props.page_break_before = Some(true);
    para_props.orphan_control = Some(2);
    para_props.outline_level = Some(3);
    para_props.padding_top = Some(Points::new(4.0));
    para_props.padding_bottom = Some(Points::new(5.0));
    para_props.padding_left = Some(Points::new(6.0));
    para_props.padding_right = Some(Points::new(7.0));

    let block = Block::StyledPara(loki_doc_model::content::block::StyledParagraph {
        style_id: None,
        direct_para_props: Some(Box::new(para_props)),
        direct_char_props: None,
        inlines: vec![Inline::Str("padded".into())],
        attr: NodeAttr::default(),
    });

    let doc = single_block_doc(block);
    let recovered = round_trip(&doc);

    let para = recovered.sections[0]
        .blocks
        .iter()
        .find_map(|b| {
            if let Block::StyledPara(p) = b {
                Some(p)
            } else {
                None
            }
        })
        .expect("StyledPara must survive round-trip");

    let pp = para
        .direct_para_props
        .as_deref()
        .expect("direct_para_props must be Some");

    assert_eq!(
        pp.page_break_before,
        Some(true),
        "page_break_before must survive"
    );
    assert_eq!(pp.orphan_control, Some(2), "orphan_control must survive");
    assert_eq!(pp.outline_level, Some(3), "outline_level must survive");
    assert!(
        (pp.padding_top.map_or(0.0, |p| p.value()) - 4.0).abs() < 0.001,
        "padding_top must survive"
    );
    assert!(
        (pp.padding_bottom.map_or(0.0, |p| p.value()) - 5.0).abs() < 0.001,
        "padding_bottom must survive"
    );
    assert!(
        (pp.padding_left.map_or(0.0, |p| p.value()) - 6.0).abs() < 0.001,
        "padding_left must survive"
    );
    assert!(
        (pp.padding_right.map_or(0.0, |p| p.value()) - 7.0).abs() < 0.001,
        "padding_right must survive"
    );
}
