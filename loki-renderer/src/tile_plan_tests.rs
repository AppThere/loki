// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the render-list planner. Extracted per the file-ceiling idiom.
//!
//! The policy itself is tested in `appthere-canvas`; what is checked here is
//! the translation — CSS boxes back to paper size — because that is where this
//! module can be wrong on its own.

use super::plan_tiles;
use appthere_canvas::residency::TextureBudget;

/// A budget nothing can exceed, so these tests isolate tile *geometry* from the
/// pressure policy. The ceiling is raised with the target — `exact` would
/// otherwise cap it at the baseline 512 MiB and quietly reintroduce pressure
/// into tests that are not about pressure.
fn huge_budget() -> TextureBudget {
    TextureBudget::with_ceiling(4 * 1024 * 1024 * 1024, 4 * 1024 * 1024 * 1024)
}

/// US Letter tile boxes in CSS px at `zoom`.
fn letter_pages(n: usize, zoom: f64) -> Vec<(usize, f64, f64)> {
    (0..n)
        .map(|i| (i, 612.0 * 96.0 / 72.0 * zoom, 792.0 * 96.0 / 72.0 * zoom))
        .collect()
}

#[test]
fn every_page_appears_in_the_render_list_mounted_or_not() {
    // The list drives layout as well as painting: an unmounted page still needs
    // its box, or the scroll geometry and the scrollbar are wrong.
    let pages = letter_pages(200, 1.0);
    let tiles = plan_tiles(&pages, 24.0, 20_000.0, 900.0, 1.0, 1.0, huge_budget());
    assert_eq!(tiles.len(), 200);
    assert!(tiles.iter().any(|t| t.mount.is_some()));
    assert!(tiles.iter().any(|t| t.mount.is_none()));
}

#[test]
fn the_css_box_survives_the_round_trip_through_points() {
    // The translation this module exists for. A page's on-screen box must come
    // back unchanged whatever the zoom, or tiles would be laid out at one size
    // and rasterised for another.
    for zoom in [0.25, 0.5, 1.0, 2.0, 4.0] {
        let pages = letter_pages(20, zoom);
        let tiles = plan_tiles(&pages, 24.0, 0.0, 900.0, zoom, 2.0, huge_budget());
        for (page, tile) in pages.iter().zip(&tiles) {
            assert_eq!((tile.w, tile.h), (page.1, page.2), "at zoom {zoom}");
        }
    }
}

#[test]
fn an_unpressured_plan_mounts_the_window_at_full_scale() {
    let pages = letter_pages(200, 1.0);
    let tiles = plan_tiles(&pages, 24.0, 20_000.0, 900.0, 1.0, 1.0, huge_budget());
    assert!(
        tiles
            .iter()
            .filter_map(|t| t.mount)
            .all(|scale| scale == 1.0),
        "nothing should be reduced with budget to spare",
    );
}

#[test]
fn a_tight_budget_reduces_scale_rather_than_the_mounted_count_first() {
    // 200% on a HiDPI display wants 157.8 MiB across three tiles. A budget just
    // under that must be met by resolution, which is L08-002's ordering.
    let pages = letter_pages(200, 2.0);
    let full = plan_tiles(&pages, 24.0, 20_000.0, 900.0, 2.0, 2.0, huge_budget());
    // A tight *target* with a ceiling well clear of it, so what this exercises is
    // the target's ordering (reduce off-centre scale before dropping tiles) and
    // not the survival regime, which is a different policy with a different test.
    let tight = plan_tiles(
        &pages,
        24.0,
        20_000.0,
        900.0,
        2.0,
        2.0,
        TextureBudget::with_ceiling(150 * 1024 * 1024, 4 * 1024 * 1024 * 1024),
    );
    let full_count = full.iter().filter(|t| t.mount.is_some()).count();
    let tight_count = tight.iter().filter(|t| t.mount.is_some()).count();
    assert_eq!(
        tight_count, full_count,
        "the tile count must not move first"
    );
    assert!(
        tight.iter().filter_map(|t| t.mount).any(|s| s < 1.0),
        "something must have been reduced",
    );
}

#[test]
fn degenerate_zoom_and_scale_factor_do_not_produce_a_nonsense_plan() {
    // Guards the division by zoom in the points round-trip. A zero or
    // non-finite zoom would otherwise produce infinite page sizes and a plan
    // that mounts nothing.
    let pages = letter_pages(20, 1.0);
    for (zoom, dsf) in [(0.0, 1.0), (f64::NAN, 1.0), (1.0, 0.0), (1.0, f64::NAN)] {
        let tiles = plan_tiles(&pages, 24.0, 0.0, 900.0, zoom, dsf, huge_budget());
        assert_eq!(tiles.len(), 20);
        assert!(
            tiles.iter().any(|t| t.mount.is_some()),
            "zoom {zoom} dsf {dsf} mounted nothing",
        );
    }
}
