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
