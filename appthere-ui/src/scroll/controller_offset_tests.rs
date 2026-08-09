// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! When a command composes against the DOM's report, and when against its own
//! previous command.

use super::{effective_offset, ScrollMetrics};

fn metrics(top: f32, left: f32) -> ScrollMetrics {
    ScrollMetrics {
        scroll_top: top,
        scroll_left: left,
        scroll_width: 400.0,
        scroll_height: 2000.0,
        client_width: 800.0,
        client_height: 600.0,
    }
}

/// With nothing commanded, the DOM's report is the answer — the ordinary case,
/// and the one every pre-gesture caller was already getting.
#[test]
fn with_no_command_the_report_wins() {
    assert_eq!(effective_offset(metrics(200.0, 10.0), None), (10.0, 200.0));
}

/// **The burst case.** A command was issued and the DOM has not reported yet —
/// the metrics are byte-for-byte the ones it was computed against — so the next
/// command composes against what was asked for, not against the stale report.
/// This is the case the sitting caught: without it, the second wheel event of a
/// notch recomputes from the position before the first one moved.
#[test]
fn an_unreported_command_supersedes_the_report() {
    let m = metrics(200.0, 10.0);
    let commanded = Some((10.0, 253.0, m));
    assert_eq!(effective_offset(m, commanded), (10.0, 253.0));
}

/// **And it stops the moment the DOM catches up.** The metrics differ from the
/// ones the command was computed against, so the report is live again. Without
/// this the memory would outlive its reason and every later command would
/// compose against an offset the container has since left.
#[test]
fn a_reported_scroll_clears_the_memory() {
    let before = metrics(200.0, 10.0);
    let commanded = Some((10.0, 253.0, before));
    let after = metrics(253.0, 10.0);
    assert_eq!(effective_offset(after, commanded), (10.0, 253.0));
    // And once the user scrolls away, the report is what counts — the remembered
    // 253 must not reappear.
    let scrolled = metrics(900.0, 10.0);
    assert_eq!(effective_offset(scrolled, commanded), (10.0, 900.0));
}

/// **A change anywhere in the metrics clears it**, not only in the offset. A
/// resize that leaves `scroll_top` alone still means the DOM has re-reported,
/// and a memory kept across it would be asserting freshness it does not have.
#[test]
fn any_metrics_change_clears_it() {
    let before = metrics(200.0, 10.0);
    let commanded = Some((10.0, 253.0, before));
    let resized = ScrollMetrics {
        client_height: 400.0,
        ..before
    };
    assert_eq!(effective_offset(resized, commanded), (10.0, 200.0));
}

/// The horizontal offset travels with the vertical one — a pointer-anchored
/// zoom commands both, and remembering only one would leave the other composing
/// against a stale report.
#[test]
fn both_axes_are_remembered() {
    let m = metrics(200.0, 10.0);
    assert_eq!(effective_offset(m, Some((77.0, 253.0, m))), (77.0, 253.0));
}
