// SPDX-License-Identifier: Apache-2.0

//! Tests for the DOM reflow view's table geometry.
//!
//! The table is T7.3's unscalable element: what it is *worth* testing here is
//! the column widths, because they are the reason it cannot be fitted to the
//! column and therefore the reason the scrollport exists.

use super::{cell_css, col_css};
use loki_doc_model::content::table::col::{ColAlignment, ColSpec, ColWidth};
use loki_doc_model::content::table::core::{Table, TableBody, TableFoot, TableHead};
use loki_doc_model::content::table::row::Cell;
use loki_doc_model::loki_primitives::units::Points;

/// A table whose one column carries `alignment`.
fn table_with(alignment: ColAlignment) -> Table {
    Table {
        borders: None,
        attr: Default::default(),
        caption: Default::default(),
        width: None,
        col_specs: vec![ColSpec {
            alignment,
            width: ColWidth::Fixed(Points::new(120.0)),
        }],
        head: TableHead::empty(),
        bodies: vec![TableBody::from_rows(Vec::new())],
        foot: TableFoot::empty(),
    }
}

/// A cell carrying `alignment` and nothing else.
fn cell_with(alignment: ColAlignment) -> Cell {
    Cell {
        attr: Default::default(),
        alignment,
        row_span: 1,
        col_span: 1,
        blocks: Vec::new(),
        props: Default::default(),
    }
}

/// **A fixed column is its points**, not a share and not a guess. This is the
/// width that makes a table unscalable, so getting it from anywhere else would
/// change what T7.3 is even about.
#[test]
fn a_fixed_column_keeps_its_points() {
    assert_eq!(col_css(&ColWidth::Fixed(Points::new(96.0))), "width: 96pt;");
}

/// A proportional column is a percentage — the same normalisation across the row
/// that `loki-layout` does over the grid.
#[test]
fn a_proportional_column_is_a_percentage() {
    let css = col_css(&ColWidth::Proportional(0.25));
    assert!(css.contains('%'), "{css}");
    assert!(
        !css.contains("pt"),
        "a share was emitted as a length: {css}"
    );
}

/// **A content-sized column gets no width at all.** The inversion of the two
/// above: emitting `width: auto` — or worse, a default number — would make the
/// DOM path decide a width the model deliberately leaves open.
#[test]
fn a_content_sized_column_is_left_alone() {
    assert_eq!(col_css(&ColWidth::Default), "");
}

/// **The cell's own alignment wins**, which is the model's order.
#[test]
fn a_cells_alignment_overrides_its_columns() {
    let table = table_with(ColAlignment::Left);
    let css = cell_css(&cell_with(ColAlignment::Right), &table, 0, false);
    assert!(css.contains("text-align: right"), "{css}");
}

/// **And the column's is used when the cell has none** — the inversion. A rule
/// that always took the cell's would silently drop every column default, and a
/// table of right-aligned numbers would come out left-aligned.
#[test]
fn a_column_default_reaches_a_cell_that_states_nothing() {
    let table = table_with(ColAlignment::Right);
    let css = cell_css(&cell_with(ColAlignment::Default), &table, 0, false);
    assert!(css.contains("text-align: right"), "{css}");
}

/// A cell in a column that does not exist falls back rather than panicking — a
/// row with more cells than the grid declares is a real document, and a render
/// is not the place to discover it.
#[test]
fn a_cell_past_the_grid_falls_back_instead_of_panicking() {
    let table = table_with(ColAlignment::Right);
    let css = cell_css(&cell_with(ColAlignment::Default), &table, 7, false);
    assert!(css.contains("text-align: start"), "{css}");
}

/// Header cells are distinguishable from body cells.
#[test]
fn a_header_cell_is_heavier_than_a_body_cell() {
    let table = table_with(ColAlignment::Default);
    let head = cell_css(&cell_with(ColAlignment::Default), &table, 0, true);
    let body = cell_css(&cell_with(ColAlignment::Default), &table, 0, false);
    assert!(head.contains("font-weight: 600"), "{head}");
    assert!(body.contains("font-weight: 400"), "{body}");
}
