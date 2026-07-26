// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`super::ScrollMetrics`].

use super::ScrollMetrics;

fn sample() -> ScrollMetrics {
    ScrollMetrics {
        scroll_top: 120.0,
        scroll_left: 30.0,
        scroll_width: 400.0,
        scroll_height: 9100.0,
        client_width: 600.0,
        client_height: 900.0,
    }
}

#[test]
fn default_is_unmeasured() {
    let m = ScrollMetrics::default();
    assert!(!m.is_measured());
    assert!(!m.can_scroll_x());
    assert!(!m.can_scroll_y());
}

#[test]
fn content_size_adds_client_to_the_scrollable_distance() {
    // The invariant the module docs warn about: scroll_height is a distance,
    // so content is client + distance. If this ever reads 9100 instead of
    // 10000, every derived figure in the scrollbar and the reveal is wrong.
    let m = sample();
    assert_eq!(m.content_height(), 10_000.0);
    assert_eq!(m.content_width(), 1_000.0);
}

#[test]
fn visible_rect_is_in_content_coordinates() {
    let m = sample();
    assert_eq!(m.visible_rect(), (30.0, 120.0, 600.0, 900.0));
}

#[test]
fn axis_scrollability_needs_more_than_half_a_pixel() {
    // Sub-pixel slack between content and client is not "scrollable"; treating
    // it as such makes the scrollbar appear on content that fits.
    let m = ScrollMetrics {
        client_width: 600.0,
        client_height: 900.0,
        scroll_width: 0.25,
        scroll_height: 0.25,
        ..Default::default()
    };
    assert!(m.is_measured());
    assert!(!m.can_scroll_x());
    assert!(!m.can_scroll_y());
}
