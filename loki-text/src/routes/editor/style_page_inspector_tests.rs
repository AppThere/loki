// SPDX-License-Identifier: Apache-2.0

//! Tests for the read-only page-style inspector rows.

use super::page_inspector_rows;
use loki_doc_model::layout::page::{
    PageLayout, PageMargins, PageOrientation, PageSize, SectionColumns,
};
use loki_doc_model::loki_primitives::units::{MeasurementUnit, Points};

/// Points is the unit these assertions are written in; the unit-dependent
/// behaviour has its own tests below.
const PT: MeasurementUnit = MeasurementUnit::Point;

fn value_for(layout: &PageLayout, key: &str) -> String {
    value_for_in(layout, key, PT)
}

fn value_for_in(layout: &PageLayout, key: &str, unit: MeasurementUnit) -> String {
    page_inspector_rows(layout, unit)
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
    // Points now carry one decimal, like every other unit: the entry field seeds
    // itself from this formatter, so whole-point rounding would make a seeded
    // value that does not read back as the page it came from.
    assert_eq!(value_for(&layout, "style-page-size"), "400.0 × 600.0 pt");
}

#[test]
fn uniform_margins_collapse_to_one_value() {
    // Default margins are a uniform 72 pt.
    let layout = PageLayout::default();
    assert_eq!(value_for(&layout, "style-page-margins"), "72.0 pt");

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
        "72.0 / 72.0 / 144.0 / 144.0 pt"
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

/// A named paper keeps its name in every unit — the size row reports the paper,
/// and only a *user-defined* size falls through to measured dimensions.
#[test]
fn a_catalogued_paper_is_named_regardless_of_unit() {
    let layout = PageLayout {
        page_size: PageSize::a4(),
        ..Default::default()
    };
    for unit in MeasurementUnit::ALL.iter().copied() {
        assert_eq!(value_for_in(&layout, "style-page-size", unit), "A4");
    }
}

/// **Margins follow the measurement unit** (T6.4), and the number changes with
/// it — an implementation that swapped only the suffix would pass a test that
/// checked the suffix alone.
#[test]
fn margins_are_shown_in_the_active_unit() {
    let layout = PageLayout::default(); // 72 pt (1 in) on every edge
    assert_eq!(value_for_in(&layout, "style-page-margins", PT), "72.0 pt");
    assert_eq!(
        value_for_in(&layout, "style-page-margins", MeasurementUnit::Inch),
        "1.00 in"
    );
    assert_eq!(
        value_for_in(&layout, "style-page-margins", MeasurementUnit::Millimeter),
        "25.4 mm"
    );
    assert_eq!(
        value_for_in(&layout, "style-page-margins", MeasurementUnit::Centimeter),
        "2.54 cm"
    );
    assert_eq!(
        value_for_in(&layout, "style-page-margins", MeasurementUnit::Pica),
        "6.00 pc"
    );
}

/// A user-defined page size is measured in the active unit too.
#[test]
fn a_custom_size_is_measured_in_the_active_unit() {
    let layout = PageLayout {
        page_size: PageSize {
            width: Points::new(500.0),
            height: Points::new(700.0),
        },
        ..Default::default()
    };
    assert_eq!(
        value_for_in(&layout, "style-page-size", PT),
        "500.0 × 700.0 pt"
    );
    let mm = value_for_in(&layout, "style-page-size", MeasurementUnit::Millimeter);
    assert!(mm.ends_with(" mm"), "{mm:?}");
    assert!(mm.starts_with("176.4 × 246.9"), "{mm:?}");
}

/// **Whether all four margins are "equal" must not depend on the display unit.**
/// The test is made in points before rounding, so a page whose margins differ by
/// a hair reads the same way in millimetres and in picas.
#[test]
fn margin_equality_is_decided_in_points_not_after_rounding() {
    let mut layout = PageLayout::default();
    // 72 pt vs 72.4 pt: within the half-point equality tolerance, and a
    // difference that mm rounding (1 decimal) would also hide, but picas
    // (2 decimals) would surface if equality were decided after formatting.
    layout.margins.left = Points::new(72.4);
    fn collapsed_in(layout: &PageLayout, unit: MeasurementUnit) -> bool {
        !value_for_in(layout, "style-page-margins", unit).contains('/')
    }
    for unit in MeasurementUnit::ALL.iter().copied() {
        assert!(
            collapsed_in(&layout, unit),
            "{unit:?} split near-equal margins into four values"
        );
    }
    // And a genuinely different edge splits in every unit.
    layout.margins.left = Points::new(144.0);
    for unit in MeasurementUnit::ALL.iter().copied() {
        assert!(
            !collapsed_in(&layout, unit),
            "{unit:?} collapsed four distinct margins into one"
        );
    }
}
