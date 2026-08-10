// SPDX-License-Identifier: Apache-2.0

//! The list toggle and level operations: joining seeds the default style into
//! the Loro catalog, re-toggling leaves the list, switching kinds keeps the
//! level, and Tab's guard refuses to invent a list or walk past a boundary.

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, read_document_styles};
use loki_doc_model::loro_mutation::{
    MAX_LIST_LEVEL, get_block_list_id, get_block_list_level, set_block_list,
};
use loki_doc_model::style::list_defaults::{DEFAULT_BULLET_LIST_ID, DEFAULT_NUMBERED_LIST_ID};
use loro::LoroDoc;

use super::{ListKind, change_list_level, toggle_list};
use crate::editing::cursor::{CursorState, DocumentPosition};

fn loro_with_para() -> LoroDoc {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Para(vec![Inline::Str("item".into())])];
    document_to_loro(&doc).expect("to loro")
}

fn cursor_at_block(block: usize) -> CursorState {
    CursorState {
        focus: Some(DocumentPosition::top_level(0, block, 0)),
        ..Default::default()
    }
}

#[test]
fn toggling_joins_seeds_the_catalog_and_retoggling_leaves() {
    let loro = loro_with_para();
    let cursor = cursor_at_block(0);

    assert!(toggle_list(&loro, &cursor, ListKind::Bullet));
    assert_eq!(
        get_block_list_id(&loro, 0).as_deref(),
        Some(DEFAULT_BULLET_LIST_ID)
    );
    // The default definition was seeded into the Loro catalog — the export
    // writers resolve it from there.
    let catalog = read_document_styles(&loro);
    assert!(
        catalog
            .list_styles
            .contains_key(&loki_doc_model::style::list_style::ListId::new(
                DEFAULT_BULLET_LIST_ID
            )),
        "join must seed the default style"
    );

    // Same kind again: the toggle leaves the list.
    assert!(toggle_list(&loro, &cursor, ListKind::Bullet));
    assert_eq!(get_block_list_id(&loro, 0), None);
}

#[test]
fn switching_kinds_keeps_the_level() {
    let loro = loro_with_para();
    let cursor = cursor_at_block(0);

    assert!(toggle_list(&loro, &cursor, ListKind::Bullet));
    assert!(change_list_level(&loro, &cursor, 2));
    assert_eq!(get_block_list_level(&loro, 0), 2);

    // Bullet → numbered is a switch, not a leave-then-rejoin at level 0.
    assert!(toggle_list(&loro, &cursor, ListKind::Numbered));
    assert_eq!(
        get_block_list_id(&loro, 0).as_deref(),
        Some(DEFAULT_NUMBERED_LIST_ID)
    );
    assert_eq!(get_block_list_level(&loro, 0), 2, "level was reset");
}

#[test]
fn tab_guard_refuses_plain_paragraphs_and_boundaries() {
    let loro = loro_with_para();
    let cursor = cursor_at_block(0);

    // Tab in a plain paragraph must not invent a list — `false` lets the
    // keydown arm fall through to focus traversal.
    assert!(!change_list_level(&loro, &cursor, 1));
    assert_eq!(get_block_list_id(&loro, 0), None);

    set_block_list(&loro, 0, DEFAULT_BULLET_LIST_ID, 0).expect("set");
    // Shift-Tab at level 0: already at the boundary, nothing to do.
    assert!(!change_list_level(&loro, &cursor, -1));
    assert_eq!(get_block_list_level(&loro, 0), 0);

    // Tab at the deepest level: clamped, nothing to do.
    set_block_list(&loro, 0, DEFAULT_BULLET_LIST_ID, MAX_LIST_LEVEL).expect("set");
    assert!(!change_list_level(&loro, &cursor, 1));
    assert_eq!(get_block_list_level(&loro, 0), MAX_LIST_LEVEL);
}
