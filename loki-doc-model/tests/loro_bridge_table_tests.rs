// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Native table mapping: `Block::Table` round-trips through the Loro bridge as
//! a structural skeleton plus live per-cell block lists (not an opaque blob),
//! so cell text is real CRDT state. Covers round-trip fidelity, native storage
//! (`KEY_TYPE == "table"`), the live cell-content containers, rich/empty cells,
//! spans + cell properties, and a table nested inside a cell.

use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::content::table::col::ColAlignment;
use loki_doc_model::content::table::row::{Cell, CellProps, CellVerticalAlign};
use loki_doc_model::content::table::{
    ColSpec, Row, Table, TableBody, TableCaption, TableFoot, TableHead, TableWidth,
};
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{document_to_loro, loro_to_document};
use loki_doc_model::loro_schema::{
    BLOCK_TYPE_TABLE, KEY_BLOCKS, KEY_SECTIONS, KEY_TABLE_CELLS, KEY_TYPE,
};
use loki_primitives::units::Points;

fn round_trip(doc: &Document) -> Document {
    let loro = document_to_loro(doc).expect("document_to_loro must succeed");
    loro_to_document(&loro).expect("loro_to_document must succeed")
}

fn doc_with_block(block: Block) -> Document {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![block];
    doc
}

fn para(text: &str) -> Block {
    Block::Para(vec![Inline::Str(text.into())])
}

fn text_cell(text: &str) -> Cell {
    Cell::simple(vec![para(text)])
}

/// A 2-column table: one header row + two body rows = 6 cells.
fn sample_table() -> Table {
    Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width: Some(TableWidth::Percent(100.0)),
        col_specs: vec![
            ColSpec::fixed(Points::new(72.0)),
            ColSpec::fixed(Points::new(144.0)),
        ],
        head: TableHead {
            attr: NodeAttr::default(),
            rows: vec![Row::new(vec![text_cell("H1"), text_cell("H2")])],
        },
        bodies: vec![TableBody::from_rows(vec![
            Row::new(vec![text_cell("a"), text_cell("b")]),
            Row::new(vec![text_cell("c"), text_cell("d")]),
        ])],
        foot: TableFoot::empty(),
    }
}

/// Reads the `KEY_TYPE` of the first block directly from the Loro document.
fn first_block_type(doc: &Document) -> Option<String> {
    let loro = document_to_loro(doc).ok()?;
    let sections = loro.get_list(KEY_SECTIONS);
    let sec = sections.get(0)?.into_container().ok()?.into_map().ok()?;
    let blocks = sec
        .get(KEY_BLOCKS)?
        .into_container()
        .ok()?
        .into_movable_list()
        .ok()?;
    let block = blocks.get(0)?.into_container().ok()?.into_map().ok()?;
    block
        .get(KEY_TYPE)?
        .into_value()
        .ok()?
        .into_string()
        .ok()
        .map(|s| s.to_string())
}

/// Counts the live per-cell content lists under `KEY_TABLE_CELLS`.
fn table_cell_list_len(doc: &Document) -> Option<usize> {
    let loro = document_to_loro(doc).ok()?;
    let sections = loro.get_list(KEY_SECTIONS);
    let sec = sections.get(0)?.into_container().ok()?.into_map().ok()?;
    let blocks = sec
        .get(KEY_BLOCKS)?
        .into_container()
        .ok()?
        .into_movable_list()
        .ok()?;
    let block = blocks.get(0)?.into_container().ok()?.into_map().ok()?;
    let cells = block
        .get(KEY_TABLE_CELLS)?
        .into_container()
        .ok()?
        .into_movable_list()
        .ok()?;
    Some(cells.len())
}

#[test]
fn table_stored_natively_not_opaque() {
    let block = Block::Table(Box::new(sample_table()));
    let doc = doc_with_block(block.clone());
    assert_eq!(
        first_block_type(&doc).as_deref(),
        Some(BLOCK_TYPE_TABLE),
        "a table must be a native table block, not an opaque snapshot"
    );
    assert_eq!(round_trip(&doc).sections[0].blocks[0], block);
}

#[test]
fn table_cells_are_separate_live_containers() {
    let doc = doc_with_block(Block::Table(Box::new(sample_table())));
    // 1 header row (2 cells) + 2 body rows (2 cells each) = 6 cells.
    assert_eq!(table_cell_list_len(&doc), Some(6));
}

#[test]
fn table_rich_cell_content_round_trips() {
    // A cell with multiple blocks and inline formatting. Cell text is live CRDT
    // state, so formatting survives — but, exactly as at the top level, the
    // bridge normalises `Strong` to a bold `StyledRun`, so we assert structure
    // and the bold run rather than byte-for-byte equality.
    let rich = Cell::simple(vec![
        Block::Heading(2, NodeAttr::default(), vec![Inline::Str("Title".into())]),
        Block::Para(vec![
            Inline::Str("see ".into()),
            Inline::Strong(vec![Inline::Str("this".into())]),
        ]),
    ]);
    let table = Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width: None,
        col_specs: vec![ColSpec::fixed(Points::new(72.0))],
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![Row::new(vec![rich])])],
        foot: TableFoot::empty(),
    };
    let doc = doc_with_block(Block::Table(Box::new(table)));
    assert_eq!(first_block_type(&doc).as_deref(), Some(BLOCK_TYPE_TABLE));

    let recovered = round_trip(&doc);
    let Block::Table(t) = &recovered.sections[0].blocks[0] else {
        panic!("expected a table");
    };
    let cell = &t.bodies[0].body_rows[0].cells[0];
    assert_eq!(cell.blocks.len(), 2, "cell must keep both blocks");
    assert!(matches!(&cell.blocks[0], Block::Heading(2, _, _)));
    let Block::Para(inlines) = &cell.blocks[1] else {
        panic!("expected a paragraph");
    };
    let has_bold = inlines.iter().any(|i| {
        matches!(i, Inline::StyledRun(r)
            if r.direct_props.as_ref().is_some_and(|p| p.bold == Some(true)))
    });
    assert!(
        has_bold,
        "bold formatting must survive in the cell: {inlines:?}"
    );
}

#[test]
fn table_spans_and_cell_props_round_trip() {
    // A cell carrying a col-span, row-span, alignment, and cell properties —
    // all structural metadata that must survive in the skeleton.
    let spanning = Cell {
        attr: NodeAttr::default(),
        alignment: ColAlignment::Center,
        row_span: 2,
        col_span: 2,
        blocks: vec![para("merged")],
        props: CellProps {
            vertical_align: Some(CellVerticalAlign::Middle),
            ..Default::default()
        },
    };
    let table = Table {
        attr: NodeAttr::default(),
        caption: TableCaption {
            short: None,
            full: vec![Inline::Str("A caption".into())],
        },
        width: Some(TableWidth::Fixed(360.0)),
        col_specs: vec![
            ColSpec::fixed(Points::new(72.0)),
            ColSpec::fixed(Points::new(72.0)),
        ],
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![Row::new(vec![spanning])])],
        foot: TableFoot::empty(),
    };
    let block = Block::Table(Box::new(table));
    let doc = doc_with_block(block.clone());
    assert_eq!(round_trip(&doc).sections[0].blocks[0], block);
}

#[test]
fn nested_table_in_cell_round_trips() {
    // A table whose cell contains another table — proves the cell block path
    // recurses through the native table mapping.
    let inner = Block::Table(Box::new(sample_table()));
    let outer = Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width: None,
        col_specs: vec![ColSpec::fixed(Points::new(200.0))],
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![Row::new(vec![Cell::simple(
            vec![para("outer"), inner],
        )])])],
        foot: TableFoot::empty(),
    };
    let block = Block::Table(Box::new(outer));
    let doc = doc_with_block(block.clone());
    assert_eq!(first_block_type(&doc).as_deref(), Some(BLOCK_TYPE_TABLE));
    assert_eq!(round_trip(&doc).sections[0].blocks[0], block);
}

#[test]
fn empty_table_round_trips_natively() {
    let table = Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width: None,
        col_specs: vec![],
        head: TableHead::empty(),
        bodies: vec![],
        foot: TableFoot::empty(),
    };
    let block = Block::Table(Box::new(table));
    let doc = doc_with_block(block.clone());
    assert_eq!(first_block_type(&doc).as_deref(), Some(BLOCK_TYPE_TABLE));
    assert_eq!(table_cell_list_len(&doc), Some(0));
    assert_eq!(round_trip(&doc).sections[0].blocks[0], block);
}

#[test]
fn table_among_paragraphs_keeps_position() {
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![
        para("before"),
        Block::Table(Box::new(sample_table())),
        para("after"),
    ];
    let recovered = round_trip(&doc);
    assert_eq!(recovered.sections[0].blocks.len(), 3);
    assert_eq!(recovered.sections[0].blocks, doc.sections[0].blocks);
}

#[test]
fn table_style_reference_round_trips_through_the_bridge() {
    // A table's named style is stored in its `"style"` attr, which the bridge
    // serialises as part of the table skeleton (Spec 05 4a.3 foundation).
    let mut table = sample_table();
    table.set_style_name(Some("GridTable4Accent2".into()));
    let back = round_trip(&doc_with_block(Block::Table(Box::new(table))));
    let Block::Table(t) = &back.sections[0].blocks[0] else {
        panic!("expected a table");
    };
    assert_eq!(t.style_name(), Some("GridTable4Accent2"));
}
