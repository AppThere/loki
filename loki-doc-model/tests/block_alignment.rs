// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for paragraph alignment mutations, top-level and path-aware (so
//! alignment works inside table cells) — plan 4a.2 follow-on.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::content::table::core::Table;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::{
    BlockPath, get_block_alignment, get_block_alignment_at, set_block_alignment,
    set_block_alignment_at,
};
use loro::LoroDoc;

fn doc_with_para(text: &str) -> LoroDoc {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(vec![Inline::Str(text.into())])];
    document_to_loro(&doc).expect("to loro")
}

/// A doc whose only block is a 1×1 grid table (its cell holds one paragraph).
fn doc_with_table_cell() -> LoroDoc {
    let mut table = Table::grid(1, 1);
    table.bodies[0].body_rows[0].cells[0].blocks =
        vec![Block::Para(vec![Inline::Str("cell".into())])];
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];
    document_to_loro(&doc).expect("to loro")
}

#[test]
fn top_level_alignment_defaults_to_left_then_updates() {
    let ldoc = doc_with_para("hello");
    assert_eq!(
        get_block_alignment(&ldoc, 0),
        "Left",
        "unset defaults to Left"
    );

    set_block_alignment(&ldoc, 0, "Center").expect("set center");
    assert_eq!(get_block_alignment(&ldoc, 0), "Center");

    set_block_alignment(&ldoc, 0, "Justify").expect("set justify");
    assert_eq!(get_block_alignment(&ldoc, 0), "Justify");
}

#[test]
fn top_level_alignment_survives_a_round_trip() {
    let ldoc = doc_with_para("hello");
    set_block_alignment(&ldoc, 0, "Right").expect("set right");
    let doc = loro_to_document(&ldoc).expect("rebuild");
    let Block::StyledPara(sp) = &doc.sections[0].blocks[0] else {
        panic!("expected a styled paragraph after alignment");
    };
    let align = sp.direct_para_props.as_ref().and_then(|p| p.alignment);
    assert!(
        matches!(align, Some(a) if format!("{a:?}") == "Right"),
        "alignment did not round-trip, got {align:?}",
    );
}

#[test]
fn alignment_inside_a_table_cell_is_path_aware() {
    let ldoc = doc_with_table_cell();
    let path = BlockPath::in_cell(0, 0, 0);
    assert_eq!(get_block_alignment_at(&ldoc, &path), "Left");

    set_block_alignment_at(&ldoc, &path, "Center").expect("align cell");
    assert_eq!(get_block_alignment_at(&ldoc, &path), "Center");

    // The cell paragraph really carries the alignment on reload.
    let doc = loro_to_document(&ldoc).expect("rebuild");
    let Block::Table(t) = &doc.sections[0].blocks[0] else {
        panic!("table");
    };
    let cell_para = &t.bodies[0].body_rows[0].cells[0].blocks[0];
    let align = match cell_para {
        Block::StyledPara(sp) => sp.direct_para_props.as_ref().and_then(|p| p.alignment),
        Block::Para(_) => None,
        _ => panic!("cell should hold a paragraph"),
    };
    assert!(
        matches!(align, Some(a) if format!("{a:?}") == "Center"),
        "cell alignment did not round-trip, got {align:?}",
    );
}

#[test]
fn heading_alignment_uses_the_jc_attribute() {
    // Headings store alignment as an OOXML `jc` attr, not para_props.
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Heading(
        1,
        loki_doc_model::NodeAttr::default(),
        vec![Inline::Str("Title".into())],
    )];
    let ldoc = document_to_loro(&doc).expect("to loro");
    assert_eq!(get_block_alignment(&ldoc, 0), "Left");

    set_block_alignment(&ldoc, 0, "Center").expect("center heading");
    assert_eq!(get_block_alignment(&ldoc, 0), "Center");

    // It stays a heading and carries jc="center" through a round-trip.
    let out = loro_to_document(&ldoc).expect("rebuild");
    let Block::Heading(level, attr, _) = &out.sections[0].blocks[0] else {
        panic!("expected a heading, not a promoted paragraph");
    };
    assert_eq!(*level, 1);
    assert_eq!(
        attr.kv
            .iter()
            .find(|(k, _)| k == "jc")
            .map(|(_, v)| v.as_str()),
        Some("center"),
    );
}

#[test]
fn setting_alignment_on_an_invalid_path_errors() {
    let ldoc = doc_with_para("hello");
    // Block 0 is a plain paragraph, not a table — a cell descent must fail.
    let bad = BlockPath::in_cell(0, 0, 0);
    assert!(set_block_alignment_at(&ldoc, &bad, "Center").is_err());
}
