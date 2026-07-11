// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`super::merge_drop_cap_frames`].

use super::*;
use loki_doc_model::content::block::StyledParagraph;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::style::props::drop_cap::{DropCap, DropCapLength};

fn para(text: &str, drop_cap: Option<DropCap>) -> Block {
    let direct_para_props = drop_cap.map(|dc| {
        Box::new(ParaProps {
            drop_cap: Some(dc),
            ..ParaProps::default()
        })
    });
    Block::StyledPara(StyledParagraph {
        style_id: None,
        direct_para_props,
        direct_char_props: None,
        inlines: vec![Inline::Str(text.into())],
        attr: NodeAttr::default(),
    })
}

fn drop_cap() -> DropCap {
    DropCap {
        lines: 3,
        length: DropCapLength::Chars(1),
        distance: loki_primitives::units::Points::new(0.0),
        margin: false,
    }
}

fn para_text(block: &Block) -> String {
    let Block::StyledPara(p) = block else {
        panic!("expected StyledPara");
    };
    p.inlines
        .iter()
        .map(|i| match i {
            Inline::Str(s) => s.as_str(),
            _ => "",
        })
        .collect()
}

#[test]
fn frame_merges_into_following_body() {
    let blocks = vec![para("D", Some(drop_cap())), para("ropped initial.", None)];
    let out = merge_drop_cap_frames(blocks);
    assert_eq!(out.len(), 1, "frame + body collapse into one paragraph");
    assert_eq!(para_text(&out[0]), "Dropped initial.");
    // The drop cap moved onto the merged body paragraph.
    let Block::StyledPara(p) = &out[0] else {
        panic!("expected StyledPara");
    };
    assert_eq!(
        p.direct_para_props.as_ref().and_then(|pp| pp.drop_cap),
        Some(drop_cap())
    );
}

#[test]
fn frame_without_following_paragraph_is_kept() {
    let blocks = vec![para("D", Some(drop_cap()))];
    let out = merge_drop_cap_frames(blocks);
    assert_eq!(out.len(), 1, "lone frame is preserved");
    assert_eq!(para_text(&out[0]), "D");
}

#[test]
fn non_drop_cap_paragraphs_are_unchanged() {
    let blocks = vec![para("First.", None), para("Second.", None)];
    let out = merge_drop_cap_frames(blocks);
    assert_eq!(out.len(), 2);
    assert_eq!(para_text(&out[0]), "First.");
    assert_eq!(para_text(&out[1]), "Second.");
}

#[test]
fn body_retains_its_own_paragraph_props() {
    // Body has its own direct props (no drop cap); after merge it keeps them
    // plus the injected drop cap.
    let body = Block::StyledPara(StyledParagraph {
        style_id: Some(loki_doc_model::style::catalog::StyleId::new("BodyStyle")),
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![Inline::Str("ropped.".into())],
        attr: NodeAttr::default(),
    });
    let blocks = vec![para("D", Some(drop_cap())), body];
    let out = merge_drop_cap_frames(blocks);
    let Block::StyledPara(p) = &out[0] else {
        panic!("expected StyledPara");
    };
    assert_eq!(
        p.style_id.as_ref().map(loki_doc_model::StyleId::as_str),
        Some("BodyStyle")
    );
    assert!(p.direct_para_props.as_ref().unwrap().drop_cap.is_some());
}
