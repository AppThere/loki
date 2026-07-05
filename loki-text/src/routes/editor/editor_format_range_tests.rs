// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::document_to_loro;
use loki_doc_model::{BlockPath, PathStep};

use super::resolve_format_ranges;
use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::routes::editor::editor_formatting::toggle_bold;

fn para(s: &str) -> Block {
    Block::Para(vec![Inline::Str(s.into())])
}

fn loro_with_paras(texts: &[&str]) -> loro::LoroDoc {
    let mut doc = Document::new();
    doc.sections[0].blocks = texts.iter().map(|t| para(t)).collect();
    document_to_loro(&doc).unwrap()
}

fn selection(anchor: DocumentPosition, focus: DocumentPosition) -> CursorState {
    let mut cs = CursorState::new();
    cs.anchor = Some(anchor);
    cs.focus = Some(focus);
    cs
}

#[test]
fn point_cursor_expands_to_the_word() {
    let loro = loro_with_paras(&["hello world"]);
    let mut cs = CursorState::new();
    cs.anchor = Some(DocumentPosition::top_level(0, 0, 7));
    cs.focus = cs.anchor.clone();
    let ranges = resolve_format_ranges(&loro, &cs);
    assert_eq!(ranges, vec![(BlockPath::block(0), 6, 11)]);
}

#[test]
fn single_paragraph_selection_is_one_range() {
    let loro = loro_with_paras(&["hello world"]);
    let cs = selection(
        DocumentPosition::top_level(0, 0, 8),
        DocumentPosition::top_level(0, 0, 2),
    );
    let ranges = resolve_format_ranges(&loro, &cs);
    assert_eq!(ranges, vec![(BlockPath::block(0), 2, 8)]);
}

#[test]
fn multi_paragraph_selection_covers_every_block() {
    let loro = loro_with_paras(&["Hello world", "middle", "goodbye"]);
    // From byte 6 of block 0 to byte 4 of block 2, endpoints reversed.
    let cs = selection(
        DocumentPosition::top_level(0, 2, 4),
        DocumentPosition::top_level(0, 0, 6),
    );
    let ranges = resolve_format_ranges(&loro, &cs);
    assert_eq!(
        ranges,
        vec![
            (BlockPath::block(0), 6, 11), // tail of "Hello world"
            (BlockPath::block(1), 0, 6),  // all of "middle"
            (BlockPath::block(2), 0, 4),  // head of "goodbye"
        ]
    );
}

#[test]
fn selection_ending_at_offset_zero_skips_the_empty_last_range() {
    let loro = loro_with_paras(&["first", "second"]);
    let cs = selection(
        DocumentPosition::top_level(0, 0, 2),
        DocumentPosition::top_level(0, 1, 0),
    );
    let ranges = resolve_format_ranges(&loro, &cs);
    assert_eq!(ranges, vec![(BlockPath::block(0), 2, 5)]);
}

#[test]
fn selection_within_one_table_cell_covers_its_blocks() {
    use loki_doc_model::content::attr::NodeAttr;
    use loki_doc_model::content::table::core::{
        Table, TableBody, TableCaption, TableFoot, TableHead,
    };
    use loki_doc_model::content::table::row::{Cell, Row};

    let cell = Cell::simple(vec![para("alpha"), para("beta")]);
    let table = Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width: None,
        col_specs: Vec::new(),
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![Row::new(vec![cell])])],
        foot: TableFoot::empty(),
    };
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![para("intro"), Block::Table(Box::new(table))];
    let loro = document_to_loro(&doc).unwrap();

    let pos = |block: usize, byte: usize| DocumentPosition {
        page_index: 0,
        paragraph_index: 1,
        byte_offset: byte,
        path: vec![PathStep::Cell { cell: 0, block }],
    };
    let cs = selection(pos(0, 2), pos(1, 3));
    let ranges = resolve_format_ranges(&loro, &cs);
    assert_eq!(
        ranges,
        vec![
            (BlockPath::in_cell(1, 0, 0), 2, 5), // tail of "alpha"
            (BlockPath::in_cell(1, 0, 1), 0, 3), // head of "beta"
        ]
    );
}

#[test]
fn cross_container_selection_clamps_to_the_focus_paragraph() {
    let loro = loro_with_paras(&["top level"]);
    let cs = selection(
        DocumentPosition {
            page_index: 0,
            paragraph_index: 0,
            byte_offset: 1,
            path: vec![PathStep::Cell { cell: 0, block: 0 }],
        },
        DocumentPosition::top_level(0, 0, 3),
    );
    let ranges = resolve_format_ranges(&loro, &cs);
    assert_eq!(ranges, vec![(BlockPath::block(0), 0, 3)]);
}

#[test]
fn a_table_between_selected_paragraphs_is_skipped() {
    use loki_doc_model::content::attr::NodeAttr;
    use loki_doc_model::content::table::core::{
        Table, TableBody, TableCaption, TableFoot, TableHead,
    };
    use loki_doc_model::content::table::row::{Cell, Row};

    let table = Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width: None,
        col_specs: Vec::new(),
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![Row::new(vec![Cell::simple(
            vec![para("cell")],
        )])])],
        foot: TableFoot::empty(),
    };
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![para("before"), Block::Table(Box::new(table)), para("after")];
    let loro = document_to_loro(&doc).unwrap();

    // Top-level selection across the table: block 0 tail + block 2 head; the
    // table (no text container) contributes nothing and its cell is untouched.
    let cs = selection(
        DocumentPosition::top_level(0, 0, 3),
        DocumentPosition::top_level(0, 2, 3),
    );
    let ranges = resolve_format_ranges(&loro, &cs);
    assert_eq!(
        ranges,
        vec![(BlockPath::block(0), 3, 6), (BlockPath::block(2), 0, 3)]
    );
}

#[test]
fn toggle_bold_marks_every_paragraph_of_a_multi_block_selection() {
    use loki_doc_model::loro_bridge::loro_to_document;

    let loro = loro_with_paras(&["Hello world", "middle", "goodbye"]);
    let cs = selection(
        DocumentPosition::top_level(0, 0, 6),
        DocumentPosition::top_level(0, 2, 4),
    );
    assert!(
        toggle_bold(&loro, &cs).unwrap(),
        "first toggle applies bold"
    );

    let rebuilt = loro_to_document(&loro).unwrap();
    let bold_text = |block: usize| -> String {
        let Block::Para(inlines) = &rebuilt.sections[0].blocks[block] else {
            panic!("para");
        };
        inlines
            .iter()
            .filter_map(|i| match i {
                Inline::StyledRun(r)
                    if r.direct_props
                        .as_ref()
                        .is_some_and(|p| p.bold == Some(true)) =>
                {
                    Some(r.content.iter().filter_map(|c| match c {
                        Inline::Str(s) => Some(s.as_str()),
                        _ => None,
                    }))
                }
                _ => None,
            })
            .flatten()
            .collect()
    };
    assert_eq!(bold_text(0), "world");
    assert_eq!(bold_text(1), "middle");
    assert_eq!(bold_text(2), "good");

    // Second toggle (state read at the selection start) removes it everywhere.
    assert!(
        !toggle_bold(&loro, &cs).unwrap(),
        "second toggle clears bold"
    );
    let rebuilt = loro_to_document(&loro).unwrap();
    let all_unbold = (0..3).all(|b| {
        let Block::Para(inlines) = &rebuilt.sections[0].blocks[b] else {
            panic!("para");
        };
        inlines.iter().all(|i| {
            !matches!(i, Inline::StyledRun(r)
                if r.direct_props.as_ref().is_some_and(|p| p.bold == Some(true)))
        })
    });
    assert!(all_unbold, "bold cleared across the whole selection");
}
