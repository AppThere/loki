// SPDX-License-Identifier: Apache-2.0

//! Tests for the page dialog's tab set and origin line.

use super::*;
use appthere_ui::DialogTabLayout;
use appthere_ui::responsive::Breakpoint;
use loki_doc_model::layout::page::{PageSize, PageUsage};
use loki_doc_model::loki_primitives::units::Points;

fn layout() -> PageLayout {
    PageLayout::default()
}

fn a4() -> PageLayout {
    let mut l = layout();
    l.page_size = paper_catalog::A4.portrait();
    l
}

#[test]
fn index_and_from_index_round_trip_for_every_tab() {
    for (i, tab) in PageTab::ALL.iter().enumerate() {
        assert_eq!(tab.index(), i);
        assert_eq!(PageTab::from_index(i), *tab);
    }
    assert_eq!(PageTab::from_index(99), PageTab::Borders, "saturates");
}

#[test]
fn labels_match_the_strip_order_and_length() {
    let labels = PageTab::labels();
    assert_eq!(labels.len(), PageTab::ALL.len());
    for (i, tab) in PageTab::ALL.iter().enumerate() {
        assert_eq!(labels[i], tab.label());
    }
}

#[test]
fn the_medium_inline_count_collapses_the_strip() {
    assert!(PageTab::INLINE_AT_MEDIUM < PageTab::ALL.len());
    assert_eq!(
        DialogTabLayout::for_breakpoint(
            Breakpoint::Medium,
            PageTab::ALL.len(),
            PageTab::INLINE_AT_MEDIUM
        ),
        DialogTabLayout::Overflow {
            visible: 3,
            overflow: 3
        }
    );
}

/// A catalogued size names its paper; anything else is Custom. Both cases must
/// be reachable, or the origin line reports one state forever.
#[test]
fn a_catalogued_size_reports_its_preset_and_anything_else_is_custom() {
    assert_eq!(PaperOrigin::of(&a4()), PaperOrigin::Preset { name: "A4" });

    let mut odd = layout();
    odd.page_size = PageSize {
        width: Points::new(123.0),
        height: Points::new(456.0),
    };
    assert_eq!(PaperOrigin::of(&odd), PaperOrigin::Custom);
}

/// Note 12: width and height are read-only under a preset and editable under
/// Custom — the predicate has to separate the two.
#[test]
fn only_a_preset_locks_the_size_boxes() {
    assert!(PaperOrigin::of(&a4()).is_preset());
    assert!(!PaperOrigin::Custom.is_preset());
}

/// The line names the paper when there is one and states the size either way,
/// so the user always sees the numbers the label stands for.
#[test]
fn the_origin_line_states_the_size_in_both_states() {
    let mm = MeasurementUnit::Millimeter;
    let preset = PaperOrigin::of(&a4()).line(&a4(), mm);
    assert!(preset.contains("A4"), "names the paper: {preset}");
    assert!(preset.contains("210"), "states the width: {preset}");

    let mut odd = layout();
    odd.page_size = PageSize {
        width: Points::new(300.0),
        height: Points::new(500.0),
    };
    let custom = PaperOrigin::Custom.line(&odd, mm);
    assert!(!custom.contains("A4"));
    // 300 pt is 105.83 mm, and the line quotes one decimal — the precision the
    // size boxes above it use, so the two cannot disagree.
    assert!(custom.contains("105.8"), "states the width in mm: {custom}");
}

/// The size in the line follows the display unit, or the origin line and the
/// boxes above it would quote different numbers for one page.
#[test]
fn the_origin_line_follows_the_display_unit() {
    let mm = PaperOrigin::of(&a4()).line(&a4(), MeasurementUnit::Millimeter);
    let inches = PaperOrigin::of(&a4()).line(&a4(), MeasurementUnit::Inch);
    assert_ne!(mm, inches);
    assert!(inches.contains("8.2") || inches.contains("8.3"), "{inches}");
}

/// Note 14: the labels follow the model's mirroring, not the screen edge.
#[test]
fn margin_labels_follow_mirroring() {
    assert_ne!(start_margin_label(true), start_margin_label(false));
    assert_ne!(end_margin_label(true), end_margin_label(false));
    assert_ne!(start_margin_label(true), end_margin_label(true));
}

/// Only `Mirrored` alternates margins. `Left` and `Right` restrict which pages
/// a layout is used for — the distinction the model warns is easy to get wrong.
#[test]
fn only_the_mirrored_usage_mirrors_margins() {
    let mut l = layout();
    l.page_usage = PageUsage::Mirrored;
    assert!(is_mirrored(&l));

    for usage in [PageUsage::All, PageUsage::Left, PageUsage::Right] {
        l.page_usage = usage;
        assert!(!is_mirrored(&l), "{usage:?} does not mirror margins");
    }
}

/// Note 13: equality is decided in points, so it cannot flip with the display
/// unit — 25.4 mm and 1 in are the same margin however they are rounded.
#[test]
fn margin_equality_is_decided_in_points_not_display_units() {
    let mut l = layout();
    for m in [&mut l.margins.top, &mut l.margins.bottom] {
        *m = Points::new(72.0);
    }
    l.margins.left = Points::new(72.0);
    l.margins.right = Points::new(72.0);
    assert!(margins_are_equal(&l));

    l.margins.left = Points::new(90.0);
    assert!(!margins_are_equal(&l), "an unequal margin is detected");
}

/// Every unit has its own suffix, or two of them would be indistinguishable
/// beside a number.
#[test]
fn every_unit_has_a_distinct_suffix() {
    let labels: Vec<String> = MeasurementUnit::ALL
        .iter()
        .map(|u| unit_label(*u))
        .collect();
    for (i, a) in labels.iter().enumerate() {
        assert!(!a.is_empty());
        for b in &labels[i + 1..] {
            assert_ne!(a, b);
        }
    }
}

/// The checkbox is labelled "Different first page", so ticked must mean the
/// distinct band **exists**. This was inverted end to end — the box showed
/// ticked when there was no variant, and ticking it deleted one — so the
/// predicate and the mutation are pinned against each other here.
#[test]
fn a_ticked_variant_box_means_the_band_exists() {
    for header in [true, false] {
        for variant in [Variant::First, Variant::Even] {
            let mut l = layout();
            assert!(
                !band_differs(&l, header, variant),
                "a fresh layout has no {variant:?} band (header={header})"
            );

            set_band_variant(&mut l, header, variant, true);
            assert!(
                band_differs(&l, header, variant),
                "asking for a distinct {variant:?} band creates one (header={header})"
            );

            set_band_variant(&mut l, header, variant, false);
            assert!(
                !band_differs(&l, header, variant),
                "clearing it removes it again (header={header})"
            );
        }
    }
}

/// Each toggle addresses exactly one of the four bands: the header tab must not
/// reach the footer's variants, and First must not reach Even.
#[test]
fn each_variant_toggle_touches_only_its_own_band() {
    let mut l = layout();
    set_band_variant(&mut l, true, Variant::First, true);

    assert!(band_differs(&l, true, Variant::First));
    assert!(
        !band_differs(&l, true, Variant::Even),
        "not the even header"
    );
    assert!(!band_differs(&l, false, Variant::First), "not the footer");
    assert!(!band_differs(&l, false, Variant::Even));
}
