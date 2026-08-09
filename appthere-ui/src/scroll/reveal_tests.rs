// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`super::reveal_offset`]. Extracted per the file-ceiling idiom.

use super::{reveal_offset, RevealMargin, CARET_LEADING_LINES, CARET_TRAILING_LINES};

/// A 900 px viewport over 10 000 px of content: max_scroll = 9100.
const CLIENT: f32 = 900.0;
const MAX: f32 = 9100.0;

fn no_margin() -> RevealMargin {
    RevealMargin::default()
}

#[test]
fn fully_visible_target_does_not_scroll() {
    // Caret mid-viewport — the common case while typing. Must be a no-op, or
    // every keystroke would jitter the page.
    assert_eq!(
        reveal_offset(1000.0, CLIENT, MAX, 1400.0, 20.0, no_margin()),
        None
    );
}

#[test]
fn target_below_the_fold_scrolls_the_minimum() {
    // Visible [1000, 1900). Target at 1950..1970 needs the bottom edge at 1970.
    let out = reveal_offset(1000.0, CLIENT, MAX, 1950.0, 20.0, no_margin());
    assert_eq!(out, Some(1970.0 - CLIENT));
}

#[test]
fn target_above_the_fold_scrolls_the_minimum() {
    // Visible [1000, 1900). Target at 800 needs the top edge at 800.
    let out = reveal_offset(1000.0, CLIENT, MAX, 800.0, 20.0, no_margin());
    assert_eq!(out, Some(800.0));
}

/// One body line, in px, at the size these tests measure against.
const LINE: f32 = 20.0;

#[test]
fn caret_margin_is_derived_from_the_named_constants() {
    // T1.9: the lookahead is one number, changed in one place. A call site that
    // re-derived either edge from a literal would pass every other test in this
    // file and silently uncouple the trigger from the rest position.
    let margin = RevealMargin::caret_lines(LINE);
    assert_eq!(margin.leading, LINE * CARET_LEADING_LINES);
    assert_eq!(margin.trailing, LINE * CARET_TRAILING_LINES);
}

#[test]
fn trailing_margin_is_kept_below_the_target() {
    // The caret-follow case: a caret at the very bottom of the viewport is
    // technically visible, but the trailing lines are not, so we scroll.
    let margin = RevealMargin::caret_lines(LINE);
    let trailing = LINE * CARET_TRAILING_LINES;
    let caret_top = 1000.0 + CLIENT - LINE; // last line of the visible band
    let out = reveal_offset(1000.0, CLIENT, MAX, caret_top, LINE, margin);
    let expected = caret_top + LINE + trailing - CLIENT;
    assert_eq!(out, Some(expected));
    // And the caret comes to rest with exactly its trailing lines below it —
    // trigger and rest are the same value, which is what makes the view
    // stair-step by a line instead of lurching.
    let new_bottom = expected + CLIENT;
    assert_eq!(new_bottom - (caret_top + LINE), trailing);
}

#[test]
fn phase_1_acceptance_two_trailing_lines_survive_at_the_page_bottom() {
    // Spec 08 Phase 1 acceptance: continuous typing at the bottom of a page
    // keeps at least two body lines of trailing space. Pinned here so lowering
    // CARET_TRAILING_LINES below 2 fails the criterion rather than the feel.
    let margin = RevealMargin::caret_lines(LINE);
    let caret_top = 1000.0 + CLIENT - LINE;
    let out = reveal_offset(1000.0, CLIENT, MAX, caret_top, LINE, margin)
        .expect("a caret on the last visible line must reveal");
    let trailing_space = (out + CLIENT) - (caret_top + LINE);
    assert!(
        trailing_space >= 2.0 * LINE,
        "acceptance needs >=2 lines below the caret, got {trailing_space} px"
    );
}

#[test]
fn leading_margin_is_kept_above_the_target() {
    let margin = RevealMargin::caret_lines(20.0);
    // Caret just below the top edge: visible, but with no line above it.
    let out = reveal_offset(1000.0, CLIENT, MAX, 1005.0, 20.0, margin);
    assert_eq!(out, Some(1005.0 - 20.0));
}

#[test]
fn clamps_at_the_top_of_the_document() {
    // A caret on line 1 wants to scroll above 0; the clamp holds it at 0.
    let margin = RevealMargin::caret_lines(20.0);
    let out = reveal_offset(50.0, CLIENT, MAX, 10.0, 20.0, margin);
    assert_eq!(out, Some(0.0));
}

#[test]
fn clamps_at_the_bottom_and_then_stops_asking() {
    let margin = RevealMargin::caret_lines(20.0);
    // Caret on the very last line, already scrolled to the end. The desired
    // offset exceeds max_scroll, clamps back to where we already are, and must
    // therefore report "no scroll needed" — not a scroll to the same place.
    let at_end = MAX;
    let caret_top = at_end + CLIENT - 30.0;
    let out = reveal_offset(at_end, CLIENT, MAX, caret_top, 20.0, margin);
    assert_eq!(out, None, "a clamped no-op must not re-issue a scroll");
}

#[test]
fn target_taller_than_the_viewport_anchors_its_leading_edge() {
    // A selection (or a caret whose margins exceed the viewport) that cannot
    // fit: the top of the target must win, so the caret stays on screen.
    let out = reveal_offset(0.0, CLIENT, MAX, 2000.0, 2000.0, no_margin());
    assert_eq!(out, Some(2000.0));
}

#[test]
fn margins_exceeding_the_viewport_still_show_the_caret() {
    // Pathological: the caret's own margins exceed the viewport at a huge line
    // height (400 leading + 20 caret + 800 trailing > 900 client).
    let margin = RevealMargin::caret_lines(400.0);
    let out = reveal_offset(0.0, CLIENT, MAX, 3000.0, 20.0, margin);
    assert_eq!(out, Some(3000.0 - 400.0));
}

#[test]
fn unmeasured_container_is_a_no_op() {
    // Before the first scroll event the client size is 0; a reveal request must
    // be dropped, not acted on with a guessed viewport.
    assert_eq!(
        reveal_offset(0.0, 0.0, 0.0, 5000.0, 20.0, no_margin()),
        None
    );
}

#[test]
fn sub_pixel_differences_do_not_scroll() {
    // Visible [1000, 1900); target ends 0.2 px past the fold. Not worth a
    // scroll event, and issuing one would loop on every keystroke.
    let out = reveal_offset(1000.0, CLIENT, MAX, 1880.0, 20.2, no_margin());
    assert_eq!(out, None);
}

#[test]
fn non_scrollable_content_never_scrolls() {
    // Content shorter than the viewport: max_scroll is 0, so every request
    // clamps to 0 and a caret already at 0 gets None.
    assert_eq!(
        reveal_offset(0.0, CLIENT, 0.0, 500.0, 20.0, no_margin()),
        None
    );
}

#[test]
fn horizontal_axis_behaves_identically() {
    // The function is axis-agnostic; this pins that it is genuinely reusable
    // for the wide-page pan case rather than vertical-only by accident.
    let out = reveal_offset(0.0, 600.0, 400.0, 700.0, 10.0, RevealMargin::new(0.0, 0.0));
    assert_eq!(out, Some(110.0));
}
