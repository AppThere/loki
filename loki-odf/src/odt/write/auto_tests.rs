// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the ODT automatic-style collector.

use super::*;
use loki_primitives::color::RgbColor;

fn color(r: u8, g: u8, b: u8) -> DocumentColor {
    DocumentColor::Rgb(RgbColor::new(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
    ))
}

#[test]
fn cell_style_emits_background_and_dedupes() {
    let mut a = AutoStyles::new();
    let n1 = a
        .cell_style(
            Some(&color(0x44, 0x72, 0xC4)),
            &CellEdges::default(),
            &Default::default(),
        )
        .expect("shaded cell → style");
    // Same colour reuses the same style name.
    let n2 = a
        .cell_style(
            Some(&color(0x44, 0x72, 0xC4)),
            &CellEdges::default(),
            &Default::default(),
        )
        .unwrap();
    assert_eq!(n1, n2);
    // A different colour gets a distinct name.
    let n3 = a
        .cell_style(
            Some(&color(0xFF, 0x00, 0x00)),
            &CellEdges::default(),
            &Default::default(),
        )
        .unwrap();
    assert_ne!(n1, n3);

    let xml = a.render();
    assert!(xml.contains(r#"style:family="table-cell""#));
    assert!(xml.contains(r##"fo:background-color="#4472C4""##));
    assert!(xml.contains(r##"fo:background-color="#FF0000""##));
}

#[test]
fn a_cell_without_shading_or_edges_gets_no_style() {
    let mut a = AutoStyles::new();
    assert_eq!(
        a.cell_style(None, &CellEdges::default(), &Default::default()),
        None
    );
    assert!(a.render().is_empty());
}

#[test]
fn resolved_edges_alone_mint_a_cell_style() {
    // The Table Grid case: the cell carries no direct formatting at all,
    // and everything it draws comes from the table style's resolved edges.
    // Before the resolution was threaded through, this cell produced no
    // style and the grid vanished on export.
    use loki_doc_model::style::props::border::{Border, BorderStyle};
    use loki_primitives::units::Points;
    let edge = |w: f64| {
        Some(Border {
            style: BorderStyle::Solid,
            width: Points::new(w),
            color: None,
            spacing: None,
        })
    };
    let mut a = AutoStyles::new();
    let name = a
        .cell_style(
            None,
            &(edge(1.0), edge(2.0), edge(3.0), edge(4.0)),
            &Default::default(),
        )
        .expect("resolved edges must produce a cell style");
    let xml = a.render();
    assert!(xml.contains(&name));
    // All four sides present, and mapped to the right ODF attribute —
    // `edges` is (top, right, bottom, left), so a tuple-order slip would
    // put the 2pt edge on `fo:border-bottom`.
    assert!(xml.contains("fo:border-top=\"1pt"), "{xml}");
    assert!(xml.contains("fo:border-right=\"2pt"), "{xml}");
    assert!(xml.contains("fo:border-bottom=\"3pt"), "{xml}");
    assert!(xml.contains("fo:border-left=\"4pt"), "{xml}");
}

#[test]
fn resolved_padding_alone_mints_a_cell_style() {
    // The `w:tblCellMar` case: the cell carries no direct formatting, and its
    // whole inset comes from the table style's resolved padding. Before the
    // resolution was threaded through, this cell produced no style at all and
    // the inherited margins were dropped on export.
    use loki_primitives::units::Points;
    let mut a = AutoStyles::new();
    let name = a
        .cell_style(
            None,
            &CellEdges::default(),
            &(
                Some(Points::new(1.0)),
                Some(Points::new(2.0)),
                Some(Points::new(3.0)),
                Some(Points::new(4.0)),
            ),
        )
        .expect("resolved padding must produce a cell style");
    let xml = a.render();
    assert!(xml.contains(&name));
    // The tuple is (top, bottom, left, right) — distinct values so a
    // mis-ordered mapping fails here rather than silently swapping sides.
    assert!(xml.contains("fo:padding-top=\"1pt"), "{xml}");
    assert!(xml.contains("fo:padding-bottom=\"2pt"), "{xml}");
    assert!(xml.contains("fo:padding-left=\"3pt"), "{xml}");
    assert!(xml.contains("fo:padding-right=\"4pt"), "{xml}");
}
