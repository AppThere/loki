// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for table serialisation.

use crate::content::render_content;
use loki_doc_model::Document;
use loki_doc_model::content::block::Block;
use loki_doc_model::content::inline::Inline;
use loki_doc_model::content::table::col::{ColAlignment, ColSpec, ColWidth};
use loki_doc_model::content::table::core::{Table, TableBody, TableCaption, TableFoot, TableHead};
use loki_doc_model::content::table::row::{Cell, CellVerticalAlign, Row};
use loki_primitives::units::Points;

/// Wraps a single block-bearing table into a one-section document and renders it.
fn render_table(table: Table) -> String {
    let mut doc = Document::new();
    let sec = doc.first_section_mut().unwrap();
    sec.blocks.clear();
    sec.blocks.push(Block::Table(Box::new(table)));
    render_content(&doc).body
}

fn cell(text: &str) -> Cell {
    Cell::simple(vec![Block::Para(vec![Inline::Str(text.into())])])
}

fn bare_table(rows: Vec<Row>) -> Table {
    Table {
        attr: Default::default(),
        caption: Default::default(),
        width: None,
        col_specs: Vec::new(),
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(rows)],
        foot: TableFoot::empty(),
    }
}

#[test]
fn renders_table_with_header_and_body() {
    let table = Table {
        head: TableHead {
            attr: Default::default(),
            rows: vec![Row::new(vec![cell("H")])],
        },
        ..bare_table(vec![Row::new(vec![cell("C")])])
    };

    let body = render_table(table);
    assert!(body.contains("<table>"));
    assert!(body.contains("<thead>"));
    assert!(body.contains("<th><p>H</p>"));
    assert!(body.contains("<td><p>C</p>"));
    assert!(body.contains("</tbody>"));
}

#[test]
fn renders_caption() {
    let table = Table {
        caption: TableCaption {
            short: None,
            full: vec![Inline::Str("Table 1: Results".into())],
        },
        ..bare_table(vec![Row::new(vec![cell("C")])])
    };
    let body = render_table(table);
    assert!(
        body.contains("<caption>Table 1: Results</caption>"),
        "caption missing: {body}"
    );
}

#[test]
fn renders_colgroup_with_fixed_and_proportional_widths() {
    let table = Table {
        col_specs: vec![
            ColSpec::fixed(Points::new(72.0)),
            ColSpec::proportional(1.0),
            ColSpec::proportional(3.0),
        ],
        ..bare_table(vec![Row::new(vec![cell("a"), cell("b"), cell("c")])])
    };
    let body = render_table(table);
    assert!(body.contains("<colgroup>"), "colgroup missing: {body}");
    assert!(
        body.contains("width:72.00pt"),
        "fixed width missing: {body}"
    );
    // 1.0 / (1.0 + 3.0) = 25%, 3.0 / 4.0 = 75%.
    assert!(body.contains("width:25.00%"), "1-share width wrong: {body}");
    assert!(body.contains("width:75.00%"), "3-share width wrong: {body}");
}

#[test]
fn colgroup_omitted_when_no_widths() {
    // All-default widths must not emit an inert colgroup of bare <col/>s.
    let table = Table {
        col_specs: vec![
            ColSpec {
                alignment: ColAlignment::Center,
                width: ColWidth::Default,
            },
            ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Default,
            },
        ],
        ..bare_table(vec![Row::new(vec![cell("a"), cell("b")])])
    };
    let body = render_table(table);
    assert!(!body.contains("<colgroup>"), "unexpected colgroup: {body}");
}

#[test]
fn cell_inherits_column_alignment() {
    let table = Table {
        col_specs: vec![
            ColSpec {
                alignment: ColAlignment::Right,
                width: ColWidth::Default,
            },
            ColSpec {
                alignment: ColAlignment::Center,
                width: ColWidth::Default,
            },
        ],
        ..bare_table(vec![Row::new(vec![cell("a"), cell("b")])])
    };
    let body = render_table(table);
    assert!(
        body.contains("<td style=\"text-align:right\"><p>a</p>"),
        "column-right alignment not applied: {body}"
    );
    assert!(
        body.contains("<td style=\"text-align:center\"><p>b</p>"),
        "column-center alignment not applied: {body}"
    );
}

#[test]
fn cell_alignment_overrides_column_default() {
    let mut overridden = cell("x");
    overridden.alignment = ColAlignment::Left;
    let table = Table {
        col_specs: vec![ColSpec {
            alignment: ColAlignment::Right,
            width: ColWidth::Default,
        }],
        ..bare_table(vec![Row::new(vec![overridden])])
    };
    let body = render_table(table);
    assert!(
        body.contains("text-align:left"),
        "cell override should win over column default: {body}"
    );
    assert!(
        !body.contains("text-align:right"),
        "stale column align: {body}"
    );
}

#[test]
fn cell_vertical_alignment_is_emitted() {
    let mut c = cell("v");
    c.props.vertical_align = Some(CellVerticalAlign::Middle);
    let body = render_table(bare_table(vec![Row::new(vec![c])]));
    assert!(
        body.contains("vertical-align:middle"),
        "vertical align missing: {body}"
    );
}

#[test]
fn alignment_tracks_column_index_through_colspan() {
    // A 2-col-spanning first cell must push the second cell onto column index 2.
    let mut spanning = cell("wide");
    spanning.col_span = 2;
    let table = Table {
        col_specs: vec![
            ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Default,
            },
            ColSpec {
                alignment: ColAlignment::Default,
                width: ColWidth::Default,
            },
            ColSpec {
                alignment: ColAlignment::Right,
                width: ColWidth::Default,
            },
        ],
        ..bare_table(vec![Row::new(vec![spanning, cell("tail")])])
    };
    let body = render_table(table);
    assert!(
        body.contains("colspan=\"2\""),
        "colspan not emitted: {body}"
    );
    assert!(
        body.contains("text-align:right"),
        "second cell should inherit column 2's right alignment: {body}"
    );
}

#[test]
fn renders_table_width() {
    let table = Table {
        width: Some(loki_doc_model::content::table::col::TableWidth::Percent(
            80.0,
        )),
        ..bare_table(vec![Row::new(vec![cell("c")])])
    };
    let body = render_table(table);
    assert!(
        body.contains("<table style=\"width:80.00%\">"),
        "table width missing: {body}"
    );
}
