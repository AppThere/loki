// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Table-cell paragraphs must emit editing data addressing the live cell
//! container: `block_index` = the table's block, and a `PathStep::Cell` whose
//! index matches the bridge's flat `KEY_TABLE_CELLS` order (head → bodies →
//! foot, row-major). Layout half of editable table cells (Spec 04 M4).

use loki_doc_model::PathStep;
use loki_doc_model::content::attr::NodeAttr;
use loki_doc_model::content::block::{Block, StyledParagraph};
use loki_doc_model::content::inline::Inline;
use loki_doc_model::content::table::core::{Table, TableBody, TableCaption, TableFoot, TableHead};
use loki_doc_model::content::table::row::{Cell, Row};
use loki_doc_model::document::Document;
use loki_layout::{
    DocumentLayout, FontResources, LayoutMode, LayoutOptions, PaginatedLayout, layout_document,
};

fn resources() -> FontResources {
    let mut r = FontResources::new();
    for p in [
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ] {
        if let Ok(data) = std::fs::read(p) {
            r.register_font(data);
            break;
        }
    }
    r
}

fn text_cell(s: &str) -> Cell {
    Cell::simple(vec![Block::StyledPara(StyledParagraph {
        style_id: None,
        direct_para_props: None,
        direct_char_props: None,
        inlines: vec![Inline::Str(s.into())],
        attr: NodeAttr::default(),
    })])
}

#[test]
fn table_cell_paragraphs_carry_cell_editing_path() {
    // One table (block 0): a single body row with two cells "a" | "b".
    let table = Table {
        attr: NodeAttr::default(),
        caption: TableCaption::default(),
        width: None,
        col_specs: Vec::new(),
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(vec![Row::new(vec![
            text_cell("a"),
            text_cell("b"),
        ])])],
        foot: TableFoot::empty(),
    };
    let mut doc = Document::new();
    doc.sections[0].blocks = vec![Block::Table(Box::new(table))];

    let mut r = resources();
    let DocumentLayout::Paginated(PaginatedLayout { pages, .. }) = layout_document(
        &mut r,
        &doc,
        LayoutMode::Paginated,
        1.0,
        &LayoutOptions {
            preserve_for_editing: true,
            spell: None,
        },
    ) else {
        panic!("paginated layout expected");
    };

    let cell_paras: Vec<_> = pages
        .iter()
        .filter_map(|p| p.editing_data.as_ref())
        .flat_map(|ed| ed.paragraphs.iter())
        .filter(|p| !p.path.is_empty())
        .collect();

    assert_eq!(cell_paras.len(), 2, "two cell paragraphs: {cell_paras:?}");
    for p in &cell_paras {
        assert_eq!(p.block_index, 0, "cells owned by the table's block 0");
    }
    // Cell 0 then cell 1, in flat order, each body block 0.
    assert_eq!(
        cell_paras[0].path,
        vec![PathStep::Cell { cell: 0, block: 0 }]
    );
    assert_eq!(
        cell_paras[1].path,
        vec![PathStep::Cell { cell: 1, block: 0 }]
    );
}
