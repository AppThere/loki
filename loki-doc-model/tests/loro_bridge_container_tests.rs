// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Loro bridge round-trip tests for the native container-block mappings
//! (`loro_bridge/containers.rs`): bullet/ordered lists, block quotes, divs,
//! and figures must survive a document_to_loro → loro_to_document cycle as
//! their own variants — not as opaque snapshots and not as the pre-mapping
//! `HorizontalRule` stubs.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{
    Block, Caption, ListAttributes, ListDelimiter, ListNumberStyle,
};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};

fn round_trip_block(block: Block) -> Block {
    let mut doc = Document::new();
    doc.sections[0].blocks.push(block);
    let loro = document_to_loro(&doc).expect("document_to_loro must succeed");
    let recovered = loro_to_document(&loro).expect("loro_to_document must succeed");
    recovered.sections[0]
        .blocks
        .first()
        .expect("block must survive round-trip")
        .clone()
}

fn para(text: &str) -> Block {
    Block::Para(vec![Inline::Str(text.into())])
}

// ── Lists ─────────────────────────────────────────────────────────────────────

#[test]
fn bullet_list_roundtrips_natively() {
    let list = Block::BulletList(vec![
        vec![para("first item")],
        vec![para("second item"), para("second item, second para")],
        vec![], // empty item must survive as an item, not vanish
    ]);
    assert_eq!(round_trip_block(list.clone()), list);
}

#[test]
fn ordered_list_roundtrips_attributes_and_items() {
    let list = Block::OrderedList(
        ListAttributes {
            start_number: 7,
            style: ListNumberStyle::LowerRoman,
            delimiter: ListDelimiter::TwoParens,
        },
        vec![vec![para("vii")], vec![para("viii")]],
    );
    assert_eq!(round_trip_block(list.clone()), list);
}

#[test]
fn nested_bullet_list_roundtrips() {
    let inner = Block::BulletList(vec![vec![para("inner")]]);
    let outer = Block::BulletList(vec![vec![para("outer"), inner]]);
    assert_eq!(round_trip_block(outer.clone()), outer);
}

// ── Block quote / div ─────────────────────────────────────────────────────────

#[test]
fn block_quote_roundtrips_natively() {
    let quote = Block::BlockQuote(vec![para("quoted"), para("still quoted")]);
    assert_eq!(round_trip_block(quote.clone()), quote);
}

#[test]
fn div_roundtrips_attr_and_children() {
    let mut attr = NodeAttr {
        id: Some("sidebar-1".into()),
        ..Default::default()
    };
    attr.classes.push("sidebar".into());
    attr.kv.push(("role".into(), "note".into()));
    let div = Block::Div(attr, vec![para("div body")]);
    assert_eq!(round_trip_block(div.clone()), div);
}

// ── Figure ────────────────────────────────────────────────────────────────────

#[test]
fn figure_roundtrips_caption_and_content() {
    let attr = NodeAttr {
        id: Some("fig-1".into()),
        ..Default::default()
    };
    let figure = Block::Figure(
        attr,
        Caption {
            short: Some(vec![Inline::Str("Short".into())]),
            full: vec![para("Full caption body")],
        },
        vec![para("figure content")],
    );
    assert_eq!(round_trip_block(figure.clone()), figure);
}

// ── Regression: the old failure mode ──────────────────────────────────────────

/// The pre-mapping bridge collapsed these types to `HorizontalRule` after one
/// CRDT cycle. Guard the whole family against reintroducing that.
#[test]
fn no_container_collapses_to_horizontal_rule() {
    let blocks = vec![
        Block::BulletList(vec![vec![para("x")]]),
        Block::OrderedList(ListAttributes::default(), vec![vec![para("y")]]),
        Block::BlockQuote(vec![para("z")]),
        Block::Div(NodeAttr::default(), vec![para("w")]),
        Block::Figure(NodeAttr::default(), Caption::default(), vec![para("v")]),
    ];
    for block in blocks {
        let recovered = round_trip_block(block.clone());
        assert!(
            !matches!(recovered, Block::HorizontalRule),
            "{block:?} must not collapse to HorizontalRule"
        );
        assert_eq!(recovered, block);
    }
}
