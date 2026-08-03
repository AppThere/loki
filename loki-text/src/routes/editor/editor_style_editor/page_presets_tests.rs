// SPDX-License-Identifier: Apache-2.0

//! Tests for the pure `apply_preset` page-geometry transform.

use super::{PagePreset, apply_preset, is_active};
use loki_doc_model::layout::page::{PageLayout, PageOrientation, PageSize};
use loki_doc_model::layout::paper_catalog::{self, PAPERS, paper_for};

#[test]
fn landscape_swaps_the_axes_and_sets_the_flag() {
    let l = apply_preset(&PageLayout::default(), PagePreset::Landscape);
    assert_eq!(l.orientation, PageOrientation::Landscape);
    assert!(l.page_size.width.value() > l.page_size.height.value());
    // Applying Portrait again restores the tall page.
    let p = apply_preset(&l, PagePreset::Portrait);
    assert_eq!(p.orientation, PageOrientation::Portrait);
    assert!(p.page_size.width.value() < p.page_size.height.value());
}

#[test]
fn size_preserves_orientation() {
    // Landscape Letter → choose A4 → stays landscape, now A4 dimensions.
    let landscape_letter = apply_preset(
        &PageLayout {
            page_size: PageSize::letter(),
            ..Default::default()
        },
        PagePreset::Landscape,
    );
    let a4 = apply_preset(&landscape_letter, PagePreset::Size(&paper_catalog::A4));
    assert!(a4.page_size.width.value() > a4.page_size.height.value());
    assert_eq!(paper_for(&a4.page_size).map(|p| p.id), Some("a4"));
}

/// **Every** catalogued paper is applicable, in both orientations — the whole
/// point of collapsing `SizeA4`/`SizeLetter` into one `Size(paper)` variant.
/// Two hardcoded variants would pass a test that only ever names two papers.
#[test]
fn every_catalogued_paper_can_be_applied_and_is_then_active() {
    let portrait = PageLayout::default();
    let landscape = apply_preset(&portrait, PagePreset::Landscape);
    for paper in PAPERS {
        for (base, want_landscape) in [(&portrait, false), (&landscape, true)] {
            let preset = PagePreset::Size(paper);
            let next = apply_preset(base, preset);
            assert_eq!(
                paper_for(&next.page_size).map(|p| p.id),
                Some(paper.id),
                "applying {} did not produce {}",
                paper.id,
                paper.id
            );
            assert_eq!(
                next.page_size.width.value() > next.page_size.height.value(),
                want_landscape,
                "applying {} changed the page orientation",
                paper.id
            );
            assert!(
                is_active(&next, preset),
                "{} is not reported active on the page it just produced",
                paper.id
            );
        }
    }
}

/// Applying one paper must clear every other paper's active state, or the form
/// would light up several size buttons at once.
#[test]
fn only_the_applied_paper_reads_as_active() {
    let a4 = apply_preset(&PageLayout::default(), PagePreset::Size(&paper_catalog::A4));
    let others = PAPERS
        .iter()
        .filter(|p| is_active(&a4, PagePreset::Size(p)))
        .count();
    assert_eq!(others, 1, "more than one size button would show as active");
}

#[test]
fn margins_presets_set_all_four_edges() {
    let wide = apply_preset(&PageLayout::default(), PagePreset::MarginsWide);
    assert_eq!(wide.margins.top.value(), 72.0);
    assert_eq!(wide.margins.left.value(), 144.0);
    // Header/footer are preserved from the default (0.5 in).
    assert_eq!(wide.margins.header.value(), 36.0);

    let narrow = apply_preset(&PageLayout::default(), PagePreset::MarginsNarrow);
    assert_eq!(narrow.margins.left.value(), 36.0);
}

#[test]
fn columns_toggle_between_single_and_multi() {
    let two = apply_preset(&PageLayout::default(), PagePreset::Columns(2));
    assert_eq!(two.columns.as_ref().map(|c| c.count), Some(2));
    // One column clears the columns entirely.
    let one = apply_preset(&two, PagePreset::Columns(1));
    assert!(one.columns.is_none());
    // Re-adding keeps a sensible default gap.
    let three = apply_preset(&PageLayout::default(), PagePreset::Columns(3));
    assert_eq!(three.columns.as_ref().map(|c| c.count), Some(3));
    assert!(three.columns.as_ref().unwrap().gap.value() > 0.0);
}

/// The stepper is what reaches counts the ribbon's three buttons cannot. Both
/// clamps are asserted from the far side: stepping down from one column and up
/// from the maximum must not move, or the control would produce a zero-column
/// or unboundedly-wide layout the model has no meaning for.
#[test]
fn the_column_stepper_clamps_at_both_ends() {
    use super::MAX_COLUMNS;

    // Up from the default single column, past the three preset buttons.
    let mut l = PageLayout::default();
    for expected in 2..=5u8 {
        l = apply_preset(&l, PagePreset::ColumnCountDelta(1));
        assert_eq!(l.columns.as_ref().map(|c| c.count), Some(expected));
    }

    // Down to one column clears `columns` entirely (the `Columns(1)` contract),
    // and stepping down again stays there rather than underflowing.
    let mut down = l;
    for _ in 0..10 {
        down = apply_preset(&down, PagePreset::ColumnCountDelta(-1));
    }
    assert!(down.columns.is_none(), "stepped below a single column");
    let still = apply_preset(&down, PagePreset::ColumnCountDelta(-1));
    assert!(still.columns.is_none());

    // Up stops at MAX_COLUMNS.
    let mut up = PageLayout::default();
    for _ in 0..(u16::from(MAX_COLUMNS) + 5) {
        up = apply_preset(&up, PagePreset::ColumnCountDelta(1));
    }
    assert_eq!(up.columns.as_ref().map(|c| c.count), Some(MAX_COLUMNS));
}

/// The separator is a modelled property both exporters write; the toggle is the
/// first UI that can set it. On a single-column layout there is no separator to
/// flip, and the toggle must not invent a `columns` block to hold one.
#[test]
fn the_separator_toggles_only_where_there_are_columns() {
    let one = apply_preset(&PageLayout::default(), PagePreset::ToggleSeparator);
    assert!(one.columns.is_none(), "toggle created a columns block");

    let two = apply_preset(&PageLayout::default(), PagePreset::Columns(2));
    assert_eq!(two.columns.as_ref().map(|c| c.separator), Some(false));
    let on = apply_preset(&two, PagePreset::ToggleSeparator);
    assert_eq!(on.columns.as_ref().map(|c| c.separator), Some(true));
    let off = apply_preset(&on, PagePreset::ToggleSeparator);
    assert_eq!(off.columns.as_ref().map(|c| c.separator), Some(false));

    // And it survives a count change, so turning the rule on then adding a
    // column does not silently drop it.
    let on_three = apply_preset(&on, PagePreset::ColumnCountDelta(1));
    assert_eq!(on_three.columns.as_ref().map(|c| c.count), Some(3));
    assert_eq!(on_three.columns.as_ref().map(|c| c.separator), Some(true));
}

/// Stepping and the preset buttons must agree on what a count means, or the
/// same layout would differ depending on which control produced it.
#[test]
fn stepping_to_three_matches_the_three_column_preset() {
    let stepped = apply_preset(
        &apply_preset(&PageLayout::default(), PagePreset::ColumnCountDelta(1)),
        PagePreset::ColumnCountDelta(1),
    );
    let preset = apply_preset(&PageLayout::default(), PagePreset::Columns(3));
    assert_eq!(stepped.columns, preset.columns);
}
