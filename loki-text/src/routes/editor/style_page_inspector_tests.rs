// SPDX-License-Identifier: Apache-2.0

//! Tests for the read-only page-style inspector rows.

use super::page_inspector_rows;
use loki_doc_model::layout::page::{
    PageLayout, PageMargins, PageOrientation, PageSize, SectionColumns,
};
use loki_doc_model::loki_primitives::units::Points;

fn value_for(layout: &PageLayout, key: &str) -> String {
    page_inspector_rows(layout)
        .into_iter()
        .find(|r| r.label_key == key)
        .map(|r| r.value)
        .unwrap_or_default()
}

#[test]
fn recognises_named_sizes_and_orientation() {
    let layout = PageLayout {
        page_size: PageSize::a4(),
        orientation: PageOrientation::Landscape,
        ..Default::default()
    };
    assert_eq!(value_for(&layout, "style-page-size"), "A4");
    assert_eq!(value_for(&layout, "style-page-orientation"), "Landscape");

    let letter = PageLayout {
        page_size: PageSize::letter(),
        ..Default::default()
    };
    assert_eq!(value_for(&letter, "style-page-size"), "US Letter");
}

/// The inspector names **every** catalogued paper, not the two it once held
/// literals for. Before T6.2 all of these rendered as `W × H pt`, so a test
/// asserting only A4 and US Letter could not tell the two implementations
/// apart.
#[test]
fn every_catalogued_paper_is_named_not_measured() {
    for paper in loki_doc_model::layout::paper_catalog::PAPERS {
        let layout = PageLayout {
            page_size: paper.portrait(),
            ..Default::default()
        };
        let shown = value_for(&layout, "style-page-size");
        assert_eq!(
            shown, paper.display_name,
            "{} rendered as {shown:?} instead of its name",
            paper.id
        );
        assert!(
            !shown.contains("pt"),
            "{} fell through to dimensions: {shown:?}",
            paper.id
        );
    }
}

/// A landscape page keeps its paper's name — the size row reports the paper,
/// and the orientation row reports the rotation.
#[test]
fn a_rotated_page_keeps_its_paper_name() {
    let a3 = loki_doc_model::layout::paper_catalog::paper_by_id("a3").expect("a3");
    let p = a3.portrait();
    let layout = PageLayout {
        page_size: PageSize {
            width: p.height,
            height: p.width,
        },
        orientation: PageOrientation::Landscape,
        ..Default::default()
    };
    assert_eq!(value_for(&layout, "style-page-size"), "A3");
}

#[test]
fn custom_size_shows_dimensions() {
    let layout = PageLayout {
        page_size: PageSize {
            width: Points::new(400.0),
            height: Points::new(600.0),
        },
        ..Default::default()
    };
    assert_eq!(value_for(&layout, "style-page-size"), "400 × 600 pt");
}

#[test]
fn uniform_margins_collapse_to_one_value() {
    // Default margins are a uniform 72 pt.
    let layout = PageLayout::default();
    assert_eq!(value_for(&layout, "style-page-margins"), "72 pt");

    let asym = PageLayout {
        margins: PageMargins {
            top: Points::new(72.0),
            bottom: Points::new(72.0),
            left: Points::new(144.0),
            right: Points::new(144.0),
            ..PageMargins::default()
        },
        ..Default::default()
    };
    assert_eq!(
        value_for(&asym, "style-page-margins"),
        "72 / 72 / 144 / 144 pt"
    );
}

#[test]
fn columns_default_to_one() {
    assert_eq!(value_for(&PageLayout::default(), "style-page-columns"), "1");
    let two = PageLayout {
        columns: Some(SectionColumns::two_column()),
        ..Default::default()
    };
    assert_eq!(value_for(&two, "style-page-columns"), "2");
}
