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
                !p.rect.overlaps(req.anchor),
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
    // Room above: 660 - 4 - 8 = 648, ample. Room below: 700 - 678 - 4 - 8 = 10.
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
