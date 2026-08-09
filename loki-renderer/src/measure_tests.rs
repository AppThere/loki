// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the ambient reading-measure cap (Spec 08 T7.2).
//!
//! The state is process-wide, so these run under one mutex and restore the cap
//! afterwards — two tests racing on a global would otherwise report each
//! other's writes, which is the one way a test of ambient state lies.

use super::{content_cap_pt, max_tile_width_px, set_content_cap_pt};
use crate::render_layout::{MAX_REFLOW_TILE_PX, PX_TO_PT, REFLOW_PADDING_PT};

/// Serialises the tests and restores the ambient cap, so neither the order they
/// run in nor the rest of the suite can see a value one of them installed.
fn with_cap<R>(cap: Option<f32>, f: impl FnOnce() -> R) -> R {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let prev = content_cap_pt();
    set_content_cap_pt(cap);
    let out = f();
    set_content_cap_pt(prev);
    out
}

/// With no measure installed the tile is the pre-T7.2 constant — the fallback
/// that keeps an app which never resolves a measure rendering exactly as before.
#[test]
fn no_measure_leaves_the_constant_in_place() {
    with_cap(None, || {
        assert!(content_cap_pt().is_none());
        assert_eq!(max_tile_width_px(), MAX_REFLOW_TILE_PX);
    });
}

/// **A narrower measure narrows the tile, by its content width plus the insets.**
/// This is the arithmetic that connects a character count to a column.
#[test]
fn a_narrower_measure_narrows_the_tile() {
    // A content column well inside the constant, so the min() below is not what
    // is being measured.
    let content_pt = 300.0;
    with_cap(Some(content_pt), || {
        let expected = (content_pt + 2.0 * REFLOW_PADDING_PT) / PX_TO_PT;
        assert!(
            (max_tile_width_px() - expected).abs() < 0.01,
            "tile {} px does not match the measure plus its insets ({expected} px)",
            max_tile_width_px()
        );
        assert!(
            max_tile_width_px() < MAX_REFLOW_TILE_PX,
            "the fixture is not actually narrower than the constant"
        );
    });
}

/// **A wider measure cannot widen the tile.** The change is one-directional on
/// purpose: a bad measurement may only shorten lines, which is harmless, never
/// lengthen them past the comfortable ceiling the reflow view already promised.
#[test]
fn a_wider_measure_is_capped_by_the_constant() {
    with_cap(Some(10_000.0), || {
        assert_eq!(
            max_tile_width_px(),
            MAX_REFLOW_TILE_PX,
            "a measure wider than the constant widened the reading column"
        );
    });
}

/// A non-finite or non-positive measure clears the cap rather than storing it.
/// Storing one would give a zero-width reading column — a blank view that reads
/// as a failed open rather than as a bad setting.
#[test]
fn an_unusable_measure_clears_the_cap_instead_of_storing_it() {
    for bad in [0.0f32, -1.0, f32::NAN, f32::INFINITY] {
        with_cap(Some(bad), || {
            assert!(
                content_cap_pt().is_none(),
                "a cap of {bad} was stored instead of cleared"
            );
            assert_eq!(max_tile_width_px(), MAX_REFLOW_TILE_PX);
        });
    }
    // The inverse: a usable value *is* stored, so the assertions above are about
    // the filter and not about `set` never working.
    with_cap(Some(300.0), || {
        assert_eq!(content_cap_pt(), Some(300.0));
    });
}
