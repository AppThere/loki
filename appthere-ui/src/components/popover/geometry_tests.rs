// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Placement, asserted as **what a reader would see** rather than as offsets.
//!
//! The R28 lesson applies directly: correct-looking factors composed on the
//! wrong side still read as correct factors, so the assertions here are
//! containment, non-overlap and side — properties a person could point at on a
//! screen — and not the intermediate x and y.
//!
//! `an_overlay_near_the_bottom_edge_stays_inside_the_viewport` was written and
//! watched to fail against the pre-T4.1 behaviour, which put 298px of a 320px
//! menu below the fold. That opportunity existed once.

use super::{place, Align, PlacementRequest, Rect, Side};

/// Room above and below the anchor, by the same arithmetic `place` uses.
///
/// Exists so a test can assert **its fixture still creates the condition it is
/// named for** before asserting the outcome. A fixture drifts silently: an
/// earlier draft of `a_container_scrolling_within_the_page_re_places_against_the_viewport`
/// used a 1400-tall viewport, so its "near the bottom" anchor still had ample
/// room below — it passed while testing nothing, and was caught only because one
/// assertion happened to name the side.
///
/// Same rule as R5a's step 2 (confirm `reduced_tiles` is non-zero before judging
/// softness) and I-06's red-before-green: **verify the fixture produces the
/// precondition, in the test, not in the author's head** (L08-044).
fn rooms(req: PlacementRequest) -> (f32, f32) {
    (
        (req.anchor.y - req.viewport.y - req.gap - req.margin).max(0.0),
        (req.viewport.bottom() - req.anchor.bottom() - req.gap - req.margin).max(0.0),
    )
}

/// A 900×700 viewport with a caret near its bottom edge — a right-click on the
/// last line of a page.
fn near_bottom() -> PlacementRequest {
    PlacementRequest {
        anchor: Rect::new(100.0, 660.0, 2.0, 18.0),
        width: 300.0,
        height: 320.0,
        viewport: Rect::new(0.0, 0.0, 900.0, 700.0),
        preferred: Side::Below,
        align: Align::Start,
        gap: 4.0,
        margin: 8.0,
    }
}

/// **The case that failed before T4.1.** A menu opened near the bottom of the
/// screen must be wholly on the screen.
#[test]
fn an_overlay_near_the_bottom_edge_stays_inside_the_viewport() {
    let req = near_bottom();
    let (above, below) = rooms(req);
    assert!(
        below < req.height && above >= req.height,
        "fixture no longer creates the condition: room below {below}, above \
         {above}, overlay {}. This test is only about the bottom edge if the \
         overlay cannot fit below and can fit above",
        req.height,
    );
    let p = place(req);
    assert!(
        p.rect.is_inside(req.viewport),
        "overlay {:?} is not inside viewport {:?} — it overflows the bottom by \
         {}px, so a menu opened near the bottom of the screen runs off it",
        p.rect,
        req.viewport,
        p.rect.bottom() - req.viewport.bottom(),
    );
    assert!(p.flipped, "the only way to fit here is above the anchor");
    assert_eq!(p.side, Side::Above);
}

/// The horizontal case the pre-T4.1 code did handle, kept so the fix is shown
/// not to have regressed it.
#[test]
fn an_overlay_near_the_right_edge_stays_inside_the_viewport() {
    let mut req = near_bottom();
    req.anchor = Rect::new(880.0, 100.0, 2.0, 18.0);
    assert!(
        req.anchor.x + req.width > req.viewport.right() - req.margin,
        "fixture must place the aligned overlay past the right edge, or the \
         horizontal case is not exercised",
    );
    let p = place(req);
    assert!(
        p.rect.is_inside(req.viewport),
        "overlay {:?} is not inside viewport {:?}",
        p.rect,
        req.viewport,
    );
    assert!(p.shifted, "alignment had to give way to the right edge");
}

/// **The failure a wrong-signed flip produces**, and the one plausible offsets
/// hide: the overlay lands on top of the control that opened it.
///
/// Swept over the whole vertical range so it cannot pass by the anchor happening
/// to sit where the arithmetic is right.
#[test]
fn an_overlay_never_covers_its_own_anchor() {
    let base = near_bottom();
    for preferred in [Side::Above, Side::Below] {
        for step in 0..=70 {
            let mut req = base;
            req.preferred = preferred;
            req.anchor = Rect::new(100.0, step as f32 * 10.0, 2.0, 18.0);
            let p = place(req);
            assert!(
                !p.rect.covers_vertically(req.anchor),
                "overlay {:?} covers its anchor {:?} (preferred {preferred:?})",
                p.rect,
                req.anchor,
            );
        }
    }
}

/// Containment holds everywhere, not only at the two edges someone thought to
/// check. Swept across anchor position, alignment and preferred side.
#[test]
fn an_overlay_is_inside_the_viewport_at_every_anchor_position() {
    let base = near_bottom();
    for preferred in [Side::Above, Side::Below] {
        for align in [Align::Start, Align::Center, Align::End] {
            for ax in 0..=90 {
                for ay in 0..=70 {
                    let mut req = base;
                    req.preferred = preferred;
                    req.align = align;
                    req.anchor = Rect::new(ax as f32 * 10.0, ay as f32 * 10.0, 24.0, 18.0);
                    let p = place(req);
                    assert!(
                        p.rect.is_inside(req.viewport),
                        "overlay {:?} escaped viewport at anchor {:?} \
                         ({preferred:?}, {align:?})",
                        p.rect,
                        req.anchor,
                    );
                }
            }
        }
    }
}

/// A flip is preferred to a truncation: a fully-visible list on the other side
/// beats a scrolling one on the preferred side.
///
/// This is the ordering the algorithm's doc comment claims, asserted rather than
/// merely stated — reversing steps 2 and 3 would truncate a menu that had
/// somewhere to go, and nothing else in this file would notice.
#[test]
fn a_flip_is_preferred_to_a_truncation() {
    let req = near_bottom();
    let (above, below) = rooms(req);
    assert!(
        below < req.height && above >= req.height,
        "fixture must offer a full-height home on exactly one side: above \
         {above}, below {below}, overlay {}",
        req.height,
    );
    let p = place(req);
    assert!(p.flipped, "expected a flip");
    assert!(
        !p.clamped,
        "the overlay fits above at full height, so it must not have been \
         truncated: {:?}",
        p.rect,
    );
    assert_eq!(p.rect.height, 320.0);
}

/// When neither side fits, the roomier one is used and the size is reduced —
/// reported, so a list can scroll rather than silently lose its tail.
#[test]
fn a_viewport_too_small_for_either_side_clamps_and_says_so() {
    let mut req = near_bottom();
    req.viewport = Rect::new(0.0, 0.0, 900.0, 200.0);
    req.anchor = Rect::new(100.0, 90.0, 2.0, 18.0);
    let (above, below) = rooms(req);
    assert!(
        above < req.height && below < req.height,
        "fixture must fit on neither side, or this tests clamping that is not \
         forced: above {above}, below {below}, overlay {}",
        req.height,
    );
    let p = place(req);
    assert!(p.clamped, "expected the height to be reduced: {:?}", p.rect);
    assert!(p.rect.is_inside(req.viewport));
    assert!(
        p.rect.height > 0.0,
        "a clamped overlay must still have some height to scroll within",
    );
    // Above has 90-4-8 = 78; below has 200-108-4-8 = 80. The roomier side wins.
    assert_eq!(p.side, Side::Below);
}

/// `End` alignment exists for T7.1: a control near the right edge whose menu
/// would be pushed off by `Start`. It must right-align *without* shifting, since
/// a shift means alignment was overridden rather than honoured.
#[test]
fn end_alignment_right_aligns_a_control_near_the_right_edge() {
    let mut req = near_bottom();
    req.anchor = Rect::new(800.0, 100.0, 60.0, 24.0);
    req.align = Align::End;
    let p = place(req);
    assert!(p.rect.is_inside(req.viewport));
    assert!(
        !p.shifted,
        "End alignment should have fitted here without a shift: {:?}",
        p.rect,
    );
    assert!(
        (p.rect.right() - req.anchor.right()).abs() < f32::EPSILON,
        "right edges should coincide: overlay {:?}, anchor {:?}",
        p.rect,
        req.anchor,
    );
}

/// A degenerate viewport must not produce an inverted rect. A window smaller
/// than twice the margin is not a real device, but it is a real transient during
/// a resize, and a negative width propagates into a style string.
#[test]
fn a_viewport_smaller_than_its_margins_stays_well_formed() {
    let mut req = near_bottom();
    req.viewport = Rect::new(0.0, 0.0, 10.0, 10.0);
    let p = place(req);
    assert!(p.rect.width >= 0.0 && p.rect.height >= 0.0, "{:?}", p.rect);
}

/// The zero-width anchor a caret is. `Center` must centre on the caret rather
/// than collapse.
#[test]
fn a_caret_anchor_has_zero_width_and_still_places() {
    let mut req = near_bottom();
    req.anchor = Rect::new(450.0, 100.0, 0.0, 18.0);
    req.align = Align::Center;
    let p = place(req);
    assert!(p.rect.is_inside(req.viewport));
    assert!(
        (p.rect.x + p.rect.width / 2.0 - 450.0).abs() < f32::EPSILON,
        "centre should sit on the caret: {:?}",
        p.rect,
    );
}

/// A preference that fits is honoured **even when the other side is roomier**.
///
/// Found by mutation: replacing `preferred_room >= wanted || preferred_room >=
/// other_room` with the roomier test alone broke no test, because every case
/// written so far had the preferred side either fitting *and* roomier, or not
/// fitting at all. The uncovered case is the one every consumer relies on —
/// T7.1's status-bar overflow opens upward because that is where it belongs, not
/// because upward happens to have more space.
#[test]
fn a_preference_that_fits_is_honoured_over_a_roomier_alternative() {
    let mut req = near_bottom();
    // Room above 388, room below 264, overlay 200: both fit, above is roomier.
    req.anchor = Rect::new(100.0, 400.0, 60.0, 24.0);
    req.height = 200.0;
    let (above, below) = rooms(req);
    assert!(
        above >= req.height && below >= req.height && above != below,
        "fixture must have BOTH sides fitting and one roomier — that is the \
         decorrelated quadrant this test exists for: above {above}, below \
         {below}, overlay {}",
        req.height,
    );
    for preferred in [Side::Above, Side::Below] {
        req.preferred = preferred;
        let p = place(req);
        assert_eq!(
            p.side, preferred,
            "a {preferred:?} preference that fits was overridden; placement must \
             follow the caller's intent, not the larger gap",
        );
        assert!(!p.flipped);
        assert!(!p.clamped);
        assert!(p.rect.is_inside(req.viewport));
    }
}

/// **The precondition M4's collapse rests on**, asserted as its consequence: if
/// *either* side can accommodate the request, the result is not truncated.
///
/// The one-comparison side selector is only correct while the required height is
/// the same on both sides. Should it ever become side-dependent — a caret drawn
/// only when opening downward is the likely cause — the selector could choose a
/// roomier side that nonetheless does not fit, and this fails. It is cheaper
/// than rediscovering the hazard as a menu that clips only when it opens upward.
///
/// Swept rather than sampled: the failure would appear in a narrow band of anchor
/// positions where the two sides' room straddles the requirement.
#[test]
fn a_request_that_fits_on_either_side_is_never_clamped() {
    let base = near_bottom();
    for preferred in [Side::Above, Side::Below] {
        for ay in 0..=70 {
            for height in [40.0_f32, 200.0, 320.0, 600.0] {
                let mut req = base;
                req.preferred = preferred;
                req.height = height;
                req.anchor = Rect::new(100.0, ay as f32 * 10.0, 2.0, 18.0);
                let above = (req.anchor.y - req.viewport.y - req.gap - req.margin).max(0.0);
                let below =
                    (req.viewport.bottom() - req.anchor.bottom() - req.gap - req.margin).max(0.0);
                if above < height && below < height {
                    continue; // Genuinely does not fit; clamping is correct.
                }
                let p = place(req);
                assert!(
                    !p.clamped,
                    "request of {height}px was truncated to {:?} although room \
                     was above={above} below={below} — the side selector chose a \
                     side that could not take it",
                    p.rect,
                );
            }
        }
    }
}

/// **Containment over generated anchors, including every kind of straddle.**
///
/// The main-axis defect below was found by a fixture in another crate, not by
/// this file — and the reason is visible in the file: every anchor here was one
/// somebody chose, and nobody chooses an anchor half off the top edge. Hand-written
/// cases cover the situations their author imagined, which is the one thing they
/// structurally cannot fix about themselves.
///
/// So the discipline that found it is imported rather than the case: the spell
/// suite's `the_menu_is_on_screen_wherever_the_click_lands` sweeps 65×41 clicks
/// and asserts one property. This does the same over anchor **rects** — positions
/// beyond all four edges, degenerate and oversized sizes, both sides, all three
/// alignments, and viewports with and without an inset — because a rect straddles
/// in ways a point cannot.
///
/// Both properties are asserted at once: the overlay stays inside the viewport,
/// and it does not cover its anchor. The second is worth sweeping here because a
/// clamp is exactly the operation that could push an overlay back onto a large
/// anchor, and no fixed case would notice.
#[test]
fn an_overlay_is_contained_for_every_generated_anchor_including_straddles() {
    let base = near_bottom();
    let viewports = [
        Rect::new(0.0, 0.0, 900.0, 700.0),
        // An inset viewport, as a safe area or a soft keyboard gives.
        Rect::new(12.0, 34.0, 876.0, 620.0),
    ];
    // Sizes: a caret, a degenerate point, an ordinary control, a row wider than
    // the overlay, and an anchor taller than the viewport itself.
    let sizes = [
        (0.0_f32, 18.0_f32),
        (0.0, 0.0),
        (60.0, 24.0),
        (400.0, 40.0),
        (2.0, 900.0),
    ];
    for vp in viewports {
        for preferred in [Side::Above, Side::Below] {
            for align in [Align::Start, Align::Center, Align::End] {
                for size in sizes {
                    for ax in (-2..=20).map(|i| i as f32 * 50.0) {
                        for ay in (-2..=16).map(|i| i as f32 * 50.0) {
                            let mut req = base;
                            req.viewport = vp;
                            req.preferred = preferred;
                            req.align = align;
                            req.anchor = Rect::new(ax, ay, size.0, size.1);
                            let p = place(req);
                            assert!(
                                p.rect.is_inside(vp),
                                "overlay {:?} escaped viewport {vp:?} for anchor \
                                 {:?} ({preferred:?}, {align:?})",
                                p.rect,
                                req.anchor,
                            );
                            assert!(
                                !p.rect.covers_vertically(req.anchor),
                                "overlay {:?} covers its anchor {:?} \
                                 ({preferred:?}, {align:?}) — a clamp pushed it \
                                 back onto the thing it belongs to",
                                p.rect,
                                req.anchor,
                            );
                        }
                    }
                }
            }
        }
    }
}

/// **The positive control the old predicate never had** — and the absence of
/// which is why it could be vacuous for a year without anyone noticing.
///
/// Every other use of this predicate is `assert!(!covers)`. A weakened predicate
/// therefore passes *everything*: found by mutation — reinstating the horizontal
/// test inside `covers_vertically` broke no test in the file, because making a
/// predicate return `false` more often cannot fail a suite that only ever asserts
/// `false`. The suite needs one case that requires it to fire.
///
/// Both cases below are that. The second also pins the deliberate
/// over-approximation: with vertical-only placement there is no "beside", so an
/// overlay sharing the anchor's vertical band is reported as covering it whatever
/// its x — the conservative direction, and the one the horizontal test destroys.
#[test]
fn the_covering_predicate_fires_when_an_overlay_is_on_top_of_a_caret() {
    let caret = Rect::new(400.0, 300.0, 0.0, 18.0);
    let on_top = Rect::new(400.0, 295.0, 300.0, 320.0);
    assert!(
        on_top.covers_vertically(caret),
        "a menu drawn straight over the caret must be reported as covering it — \
         a zero-width anchor is exactly where a rectangle intersection quietly \
         says no",
    );
    let beside = Rect::new(700.0, 295.0, 300.0, 320.0);
    assert!(
        beside.covers_vertically(caret),
        "an overlay in the caret's vertical band is a collision however its x \
         falls; `Side` has no horizontal axis for it to be legitimately beside",
    );
}

/// **What the sweep turned up: an anchor taller than its viewport has no room on
/// either side, and the result is an overlay of zero height** — a menu that
/// simply does not appear.
///
/// Recorded rather than fixed, with the reason for each half:
///
/// * It is **not silently fine**: `clamped` is true, which is the signal a
///   consumer already has to check, and a consumer that renders a
///   zero-height box gets nothing rather than something wrong.
/// * It is **not reachable for any named consumer**: a caret, a list entry, a
///   swatch, a status-bar control. None is 700px tall. Designing a concession
///   for it now would be the `Before`/`After` mistake — building for a consumer
///   nobody has named.
///
/// So the assertion is that the case stays *reported*, not that it is
/// accommodated. If a fifth consumer ever anchors to something that tall, this
/// is where the decision is written down.
#[test]
fn an_anchor_taller_than_the_viewport_yields_a_reported_empty_placement() {
    let mut req = near_bottom();
    req.anchor = Rect::new(50.0, -100.0, 2.0, 900.0);
    let (above, below) = rooms(req);
    assert!(
        above == 0.0 && below == 0.0,
        "precondition: the anchor must leave no room on either side: above \
         {above}, below {below}",
    );
    let p = place(req);
    assert_eq!(
        p.rect.height, 0.0,
        "there is nowhere to put it: {:?}",
        p.rect
    );
    assert!(
        p.clamped,
        "an unshowable placement must at least say it was reduced",
    );
}

/// **The unstated precondition, found by a real fixture.** Containment on the
/// main axis was conditional on the anchor lying inside the viewport — every
/// case above has one, so nothing here noticed.
///
/// It surfaced from `loki-text`'s spell-menu placement: a click 2px from the top
/// of an Android window whose safe area starts at 34px put the menu at y=24,
/// under the status bar. The mutation that removes the clamp passes this whole
/// file without it.
///
/// Kept beside the sweep above rather than replaced by it: the sweep is what
/// would have found this, and this is what names where it came from.
#[test]
fn an_anchor_outside_the_viewport_still_yields_a_contained_overlay() {
    let base = near_bottom();
    // A viewport inset from the window, as a safe area is.
    let vp = Rect::new(0.0, 34.0, 900.0, 742.0);
    for preferred in [Side::Above, Side::Below] {
        for ay in [-40.0_f32, -10.0, 0.0, 2.0, 20.0, 770.0, 800.0] {
            let mut req = base;
            req.viewport = vp;
            req.preferred = preferred;
            req.anchor = Rect::new(100.0, ay, 2.0, 18.0);
            let p = place(req);
            assert!(
                p.rect.is_inside(vp),
                "overlay {:?} escaped viewport {vp:?} for an anchor at y={ay} \
                 ({preferred:?}) — the anchor is outside the viewport, which is \
                 not a reason for the overlay to be",
                p.rect,
            );
        }
    }
}
