// SPDX-License-Identifier: Apache-2.0

//! Tests for the pure `apply_preset` page-geometry transform.

use super::{PagePreset, apply_preset};
use loki_doc_model::layout::page::{PageLayout, PageOrientation, PageSize};

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
    let a4 = apply_preset(&landscape_letter, PagePreset::SizeA4);
    assert!(a4.page_size.width.value() > a4.page_size.height.value());
    let (short, long) = (
        a4.page_size.width.value().min(a4.page_size.height.value()),
        a4.page_size.width.value().max(a4.page_size.height.value()),
    );
    assert!((short - PageSize::a4().width.value()).abs() < 1.0);
    assert!((long - PageSize::a4().height.value()).abs() < 1.0);
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
