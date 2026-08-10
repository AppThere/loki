// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! List membership mutations: the write must be one the reader actually
//! consumes (asserted through a full `loro_to_document` round trip, not by
//! reading the raw map back), the not-a-list guard must refuse to invent a
//! list, and headings must stay untouched.

use loro::LoroDoc;

use super::{
    MAX_LIST_LEVEL, clear_block_list_at, get_block_list_id_at, get_block_list_level,
    set_block_list, set_block_list_at, set_block_list_level, set_block_list_level_at,
};
use crate::NodeAttr;
use crate::content::block::Block;
use crate::content::inline::Inline;
use crate::content::table::core::Table;
use crate::document::Document;
use crate::loro_bridge::{document_to_loro, loro_to_document};
use crate::loro_mutation::{BlockPath, PathStep, clear_block_list, get_block_list_id};

fn doc_with_para(text: &str) -> LoroDoc {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(vec![Inline::Str(text.into())])];
    document_to_loro(&doc).expect("to loro")
}

/// The round-trip read of block 0's `(list_id, list_level)` — through the
/// bridge reader, so a write the reader ignores fails these tests.
fn read_back(loro: &LoroDoc) -> Option<(String, u8)> {
    let doc = loro_to_document(loro).expect("from loro");
    match &doc.sections[0].blocks[0] {
        Block::StyledPara(p) => {
            let props = p.direct_para_props.as_ref()?;
            let id = props.list_id.as_ref()?.as_str().to_string();
            Some((id, props.list_level.unwrap_or(0)))
        }
        _ => None,
    }
}

#[test]
fn set_block_list_survives_the_bridge_reader() {
    let loro = doc_with_para("item one");
    set_block_list(&loro, 0, "__default-bullet", 0).expect("set");
    assert_eq!(
        read_back(&loro),
        Some(("__default-bullet".to_string(), 0)),
        "the reader must see the membership the writer stored"
    );
    assert_eq!(
        get_block_list_id(&loro, 0).as_deref(),
        Some("__default-bullet")
    );
}

#[test]
fn level_changes_only_apply_to_list_items() {
    let loro = doc_with_para("plain paragraph");
    // The guard: Tab in a plain paragraph must not invent a list.
    assert_eq!(set_block_list_level(&loro, 0, 1).expect("set"), false);
    assert_eq!(read_back(&loro), None, "no membership was written");

    set_block_list(&loro, 0, "__default-numbered", 0).expect("set");
    assert_eq!(set_block_list_level(&loro, 0, 2).expect("set"), true);
    assert_eq!(
        read_back(&loro),
        Some(("__default-numbered".to_string(), 2))
    );
    assert_eq!(get_block_list_level(&loro, 0), 2);
}

#[test]
fn level_is_clamped_to_the_model_maximum() {
    let loro = doc_with_para("deep");
    set_block_list(&loro, 0, "b", 200).expect("set");
    assert_eq!(read_back(&loro).map(|(_, l)| l), Some(MAX_LIST_LEVEL));
}

#[test]
fn clear_removes_membership_and_level() {
    let loro = doc_with_para("item");
    set_block_list(&loro, 0, "b", 3).expect("set");
    clear_block_list(&loro, 0).expect("clear");
    assert_eq!(read_back(&loro), None);
    assert_eq!(get_block_list_id(&loro, 0), None);
}

#[test]
fn headings_are_left_untouched() {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Heading(
        1,
        NodeAttr::default(),
        vec![Inline::Str("Chapter".into())],
    )];
    let loro = document_to_loro(&doc).expect("to loro");
    set_block_list(&loro, 0, "b", 0).expect("set is a no-op, not an error");
    let round = loro_to_document(&loro).expect("from loro");
    assert!(
        matches!(&round.sections[0].blocks[0], Block::Heading(1, _, _)),
        "the heading must survive unchanged"
    );
}

#[test]
fn path_aware_variants_reach_a_table_cell() {
    let mut table = Table::grid(1, 1);
    table.bodies[0].body_rows[0].cells[0].blocks =
        vec![Block::Para(vec![Inline::Str("cell".into())])];
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];
    let loro = document_to_loro(&doc).expect("to loro");

    let path = BlockPath {
        root: 0,
        steps: vec![PathStep::Cell { cell: 0, block: 0 }],
    };
    set_block_list_at(&loro, &path, "cell-list", 1).expect("set at");
    assert_eq!(
        get_block_list_id_at(&loro, &path).as_deref(),
        Some("cell-list")
    );
    assert_eq!(
        set_block_list_level_at(&loro, &path, 4).expect("level at"),
        true
    );

    clear_block_list_at(&loro, &path).expect("clear at");
    assert_eq!(get_block_list_id_at(&loro, &path), None);
}
