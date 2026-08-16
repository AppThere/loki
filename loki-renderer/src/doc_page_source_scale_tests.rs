// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The zoom clamp, the capability cap, and the memo that keeps the cap's search
//! off the scroll path.

use std::sync::Arc;

use appthere_canvas::residency::{BudgetInputs, TextureBudget, ZOOM_RANGE_MAX, ZOOM_RANGE_MIN};
use loki_doc_model::document::Document;

use crate::doc_page_source::DocPageSource;
use crate::zoom_capability::CapabilityInputs;

/// The clamp must be the shared residency bound, not a literal that happens
/// to agree with it today.
///
/// It *was* a literal `0.25, 4.0` here, in a crate the residency model cannot
/// see. That mattered more than a duplicated number usually does: texture
/// demand goes as the square of zoom, so this pair decides the peak byte
/// figure any device can be asked for, and therefore whether the survival
/// regime is reachable at all.
///
/// The other half of the coupling —
/// `plan_reachability_tests::the_sweep_covers_the_whole_clamped_range` —
/// checks that the reachability sweep actually reaches this bound.
#[test]
fn the_zoom_clamp_is_the_shared_residency_bound() {
    let source = DocPageSource::new(Arc::new(Document::default()));
    source.set_zoom(1000.0);
    assert_eq!(f64::from(source.zoom()), ZOOM_RANGE_MAX);
    source.set_zoom(0.0);
    assert_eq!(f64::from(source.zoom()), ZOOM_RANGE_MIN);
    source.set_zoom(1.5);
    assert_eq!(source.zoom(), 1.5, "an in-range zoom must pass through");
}

/// A capability cap must reduce what is *rendered* without touching what was
/// *asked for* — the property that makes a forced reduction recoverable.
#[test]
fn a_capability_cap_lowers_effective_zoom_and_leaves_the_request_intact() {
    let source = DocPageSource::new(Arc::new(Document::default()));
    source.set_zoom(3.0);
    source.set_capability_limit_permille(Some(1500));
    assert_eq!(source.zoom(), 1.5, "effective zoom must follow the cap");
    assert_eq!(
        source.requested_zoom(),
        3.0,
        "the cap must not overwrite the user's setting",
    );
}

/// The half that a store-the-clamped-value design gets wrong: when the
/// constraint lifts, the reader goes back to where they were.
///
/// This is the whole reason the two are separate. Without it, memory
/// pressure at 300% strands a session at 150% permanently, and — once page
/// size varies per section (T6.1) — scrolling Letter → A2 → Letter strands
/// it at the A2 limit.
#[test]
fn lifting_the_cap_restores_the_requested_zoom() {
    let source = DocPageSource::new(Arc::new(Document::default()));
    source.set_zoom(3.0);
    source.set_capability_limit_permille(Some(1500));
    assert_eq!(source.zoom(), 1.5);
    source.set_capability_limit_permille(None);
    assert_eq!(
        source.zoom(),
        3.0,
        "removing the cap must restore the requested zoom, not leave the \
         reduced one in place",
    );
}

/// A cap above the request changes nothing — the common case on ordinary
/// paper, where the device can serve the whole range.
#[test]
fn a_cap_above_the_request_is_inert() {
    let source = DocPageSource::new(Arc::new(Document::default()));
    source.set_zoom(1.25);
    source.set_capability_limit_permille(Some(4000));
    assert_eq!(source.zoom(), 1.25);
}

/// Requests still clamp to the control's range permanently. That clamp is a
/// different kind from the capability cap — it is what the control offers,
/// not what today's device can serve — so it *should* be destructive.
#[test]
fn the_range_clamp_stays_destructive_because_it_is_not_a_capability() {
    let source = DocPageSource::new(Arc::new(Document::default()));
    source.set_zoom(99.0);
    assert_eq!(f64::from(source.requested_zoom()), ZOOM_RANGE_MAX);
}

/// **The memo skips the search, and only while the inputs hold.**
///
/// Counted rather than timed (L08-038): the question is whether `compute`
/// ran, and a counter answers it exactly where a clock would answer
/// "probably". A recompute costs up to 3040 `plan_residency` calls, so the
/// difference between one call and one per scroll frame is the whole point
/// of the memo.
///
/// **Both polarities, and the second is the one that matters** — a memo that
/// never recomputed would pass every "it ran once" assertion while pinning
/// the cap to whatever the first frame saw, so a document that grew an A2
/// page, a reader who dragged the window to a Retina display, and a memory
/// probe that lowered the budget would all be ignored.
#[test]
fn the_capability_search_runs_once_per_change_and_not_once_per_frame() {
    let source = DocPageSource::new(Arc::new(Document::default()));
    let runs = std::cell::Cell::new(0_u32);
    let letter = CapabilityInputs::from_pages(&[(612.0, 792.0)], 2.0, TextureBudget::baseline());
    let apply = |inputs| {
        source.apply_capability_limit(inputs, || {
            runs.set(runs.get() + 1);
            Some(1500)
        });
    };

    for _ in 0..64 {
        apply(letter);
    }
    assert_eq!(runs.get(), 1, "an unchanged document re-ran the search");
    assert_eq!(source.capability_limit_permille(), Some(1500));

    // Each input on its own must be able to invalidate: keying on a subset
    // would look identical here until the untracked one moved in production.
    let a2 = CapabilityInputs::from_pages(&[(1191.0, 1684.0)], 2.0, TextureBudget::baseline());
    apply(a2);
    assert_eq!(runs.get(), 2, "a larger page must re-run the search");
    let retina = CapabilityInputs {
        device_scale_factor: 3.0,
        ..a2
    };
    apply(retina);
    assert_eq!(
        runs.get(),
        3,
        "a display-scale change must re-run the search"
    );
    let leaner = CapabilityInputs {
        budget: TextureBudget::derive(BudgetInputs {
            available_ram_bytes: Some(2 * 1024 * 1024 * 1024),
            total_ram_bytes: Some(2 * 1024 * 1024 * 1024),
            gpu_paint_path: Some(true),
            user_override_bytes: None,
            diagnostic_ceiling_bytes: None,
        }),
        ..retina
    };
    apply(leaner);
    assert_eq!(runs.get(), 4, "a budget change must re-run the search");
}

/// **A direct set drops the memo**, so an override cannot leave a key
/// claiming a limit the inputs never produced.
#[test]
fn overriding_the_cap_invalidates_the_memo() {
    let source = DocPageSource::new(Arc::new(Document::default()));
    let runs = std::cell::Cell::new(0_u32);
    let letter = CapabilityInputs::from_pages(&[(612.0, 792.0)], 2.0, TextureBudget::baseline());
    let apply = || {
        source.apply_capability_limit(letter, || {
            runs.set(runs.get() + 1);
            Some(1500)
        });
    };
    apply();
    apply();
    assert_eq!(runs.get(), 1);
    source.set_capability_limit_permille(Some(4000));
    apply();
    assert_eq!(
        runs.get(),
        2,
        "the same inputs after an override must be recomputed, not assumed",
    );
    assert_eq!(source.capability_limit_permille(), Some(1500));
}
