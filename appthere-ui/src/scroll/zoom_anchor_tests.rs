// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The anchoring arithmetic.
//!
//! Every test here asks the same question in a different place: **is the content
//! that was under the anchor still under it?** That is stated directly by
//! `content_under`, rather than by comparing scroll offsets to expected numbers
//! — an expected-offset test passes for a formula that is wrong in a way the
//! test's own arithmetic repeats.

use super::{anchored_scroll, ZoomAnchor};

const VIEWPORT: (f32, f32) = (800.0, 600.0);

/// No padding between the scrollport and the scaled content — the case most of
/// these tests use, so the padding term is not doing their work for them.
const FLUSH: (f32, f32) = (0.0, 0.0);

/// This app's canvas padding, and the value the sitting measured against.
const PADDED: (f32, f32) = (0.0, 24.0);

/// The content coordinate sitting under a viewport offset.
///
/// The inverse of the layout the module documents — `a = p + d·z − s` — so a
/// formula that drops `p` disagrees with this rather than repeating its mistake.
fn content_under(scroll: f32, anchor_offset: f32, zoom: f32, padding: f32) -> f32 {
    (scroll + anchor_offset - padding) / zoom
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

/// **The property, at the viewport centre.** Zooming from a button must leave
/// the middle of the page where it was.
#[test]
fn the_centre_holds_across_a_zoom_change() {
    let scroll = (0.0, 1000.0);
    let before = content_under(scroll.1, VIEWPORT.1 / 2.0, 1.0, 0.0);
    let after_scroll = anchored_scroll(scroll, VIEWPORT, FLUSH, ZoomAnchor::Centre, 1.0, 2.0);
    let after = content_under(after_scroll.1, VIEWPORT.1 / 2.0, 2.0, 0.0);
    assert!(close(before, after), "{before} != {after}");
}

/// **And at an arbitrary pointer position**, which is the Ctrl+scroll case. A
/// formula that only holds at the centre is one that dropped the anchor term,
/// and the centre is exactly where that mistake is invisible.
#[test]
fn the_point_under_the_pointer_holds() {
    let scroll = (120.0, 940.0);
    for (ax, ay) in [(0.0, 0.0), (37.0, 11.0), (400.0, 300.0), (799.0, 599.0)] {
        let anchor = ZoomAnchor::Viewport { x: ax, y: ay };
        let bx = content_under(scroll.0, ax, 1.5, 0.0);
        let by = content_under(scroll.1, ay, 1.5, 0.0);
        let after = anchored_scroll(scroll, VIEWPORT, FLUSH, anchor, 1.5, 2.25);
        assert!(
            close(content_under(after.0, ax, 2.25, 0.0), bx),
            "x at {ax}"
        );
        assert!(
            close(content_under(after.1, ay, 2.25, 0.0), by),
            "y at {ay}"
        );
    }
}

/// **The padding is part of the model, not a correction to it.** The content
/// starts one unscaled padding below the scrollport, and that offset does not
/// grow with the zoom — so a formula that ignores it drifts by `p·(1 − r)`.
///
/// This is the case the screen sitting caught: 24 px of canvas padding and a
/// 1.21× wheel notch predicted −5.04 px and measured −5 px, which at one notch
/// reads as imprecision and across the control's range is 120 px.
#[test]
fn the_unscaled_content_offset_is_held_too() {
    let scroll = (0.0, 200.0);
    let anchor = ZoomAnchor::Viewport { x: 0.0, y: 153.0 };
    let before = content_under(scroll.1, 153.0, 1.0, PADDED.1);
    let after = anchored_scroll(scroll, VIEWPORT, PADDED, anchor, 1.0, 1.21);
    let held = content_under(after.1, 153.0, 1.21, PADDED.1);
    assert!(close(before, held), "{before} != {held}");
}

/// **And ignoring it would be visible here.** The same gesture computed with no
/// padding lands somewhere else — so the test above is discriminating, not a
/// tautology that any formula satisfies.
#[test]
fn the_content_offset_changes_the_answer() {
    let scroll = (0.0, 200.0);
    let anchor = ZoomAnchor::Viewport { x: 0.0, y: 153.0 };
    let padded = anchored_scroll(scroll, VIEWPORT, PADDED, anchor, 1.0, 1.21);
    let flush = anchored_scroll(scroll, VIEWPORT, FLUSH, anchor, 1.0, 1.21);
    // Exactly the drift the module predicts: p·(r − 1) = 24 × 0.21.
    assert!(
        close(padded.1 - flush.1, -24.0 * 0.21),
        "{padded:?} {flush:?}"
    );
}

/// A flush container (no padding) is the old behaviour exactly, so adding the
/// term did not move any caller that has none.
#[test]
fn a_flush_container_is_unchanged() {
    let scroll = (33.0, 777.0);
    let anchor = ZoomAnchor::Viewport { x: 11.0, y: 222.0 };
    let after = anchored_scroll(scroll, VIEWPORT, FLUSH, anchor, 1.0, 2.0);
    assert!(close(after.1, (777.0 + 222.0) * 2.0 - 222.0), "{after:?}");
}

/// It holds zooming **out** as well as in — the direction that a formula with a
/// flipped ratio still passes half the time.
#[test]
fn the_anchor_holds_zooming_out() {
    let scroll = (0.0, 2000.0);
    let anchor = ZoomAnchor::Viewport { x: 0.0, y: 200.0 };
    let before = content_under(scroll.1, 200.0, 3.0, 0.0);
    let after = anchored_scroll(scroll, VIEWPORT, FLUSH, anchor, 3.0, 1.0);
    assert!(close(content_under(after.1, 200.0, 1.0, 0.0), before));
    assert!(after.1 < scroll.1, "zooming out must scroll back up");
}

/// A zoom that does not change does not move the view. The identity, and the
/// case a ratio computed the wrong way round fails immediately.
#[test]
fn an_unchanged_zoom_leaves_the_scroll_alone() {
    let scroll = (33.0, 777.0);
    let after = anchored_scroll(scroll, VIEWPORT, PADDED, ZoomAnchor::Centre, 1.25, 1.25);
    assert!(
        close(after.0, scroll.0) && close(after.1, scroll.1),
        "{after:?}"
    );
}

/// **Zooming in near the top must not ask for a negative scroll.** The formula
/// produces one whenever the anchor is above the content point, and a negative
/// offset is either clamped by the container or rejected outright — either way
/// the anchor is lost, quietly.
#[test]
fn the_result_is_never_negative() {
    let after = anchored_scroll((0.0, 0.0), VIEWPORT, PADDED, ZoomAnchor::Centre, 4.0, 1.0);
    assert!(after.0 >= 0.0 && after.1 >= 0.0, "{after:?}");
    let after = anchored_scroll((10.0, 10.0), VIEWPORT, PADDED, ZoomAnchor::Centre, 8.0, 1.0);
    assert!(after.0 >= 0.0 && after.1 >= 0.0, "{after:?}");
}

/// **The upper bound is deliberately not applied here.** Clamping against the
/// *old* extent would under-scroll exactly when zooming in, which is the common
/// direction — so the result may exceed today's extent, and the container
/// clamps it against tomorrow's.
#[test]
fn zooming_in_may_ask_for_more_scroll_than_exists_today() {
    let after = anchored_scroll(
        (1000.0, 1000.0),
        VIEWPORT,
        FLUSH,
        ZoomAnchor::Centre,
        1.0,
        4.0,
    );
    assert!(
        after.1 > 1000.0,
        "zooming in must scroll further down: {after:?}"
    );
}

/// A nonsense zoom leaves the scroll untouched rather than sending the view to
/// an arbitrary place — the state before anything has been measured.
#[test]
fn a_nonsense_zoom_does_not_move_the_view() {
    let scroll = (5.0, 500.0);
    for (from, to) in [
        (0.0, 2.0),
        (2.0, 0.0),
        (-1.0, 2.0),
        (f32::NAN, 2.0),
        (2.0, f32::INFINITY),
    ] {
        assert_eq!(
            anchored_scroll(scroll, VIEWPORT, PADDED, ZoomAnchor::Centre, from, to),
            scroll,
            "{from} -> {to}",
        );
    }
}

/// The centre anchor really is the centre — a check that fails if the halving
/// is dropped, which every test above would survive because they use `Centre`'s
/// own answer on both sides.
#[test]
fn the_centre_anchor_is_the_middle_of_the_viewport() {
    assert_eq!(ZoomAnchor::Centre.offset(800.0, 600.0), (400.0, 300.0));
    assert_eq!(
        ZoomAnchor::Viewport { x: 12.0, y: 34.0 }.offset(800.0, 600.0),
        (12.0, 34.0),
    );
}
