// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Leak classifier (Spec 06 M4). The residual measurement needs a live allocator
//! (exercised by the `leak_*` benches); the verdict logic is pure and tested here.

use super::*;

const SLACK: u64 = 64 * 1024; // 64 KiB one-time/noise envelope.

#[test]
fn flat_residual_is_bounded() {
    // A clean open/edit/close leaves ~0 net either way.
    assert_eq!(classify_leak(0, 0, SLACK), LeakVerdict::Bounded);
    // One-time init paid once → within slack.
    assert_eq!(classify_leak(20_000, 25_000, SLACK), LeakVerdict::Bounded);
}

#[test]
fn residual_scaling_with_reps_is_a_leak() {
    // A retained document per cycle: 1 doc vs 64 docs of live heap.
    let one_doc = 500_000;
    let sixty_four_docs = 64 * 500_000;
    assert_eq!(
        classify_leak(one_doc, sixty_four_docs, SLACK),
        LeakVerdict::Leaking,
    );
    assert!(classify_leak(one_doc, sixty_four_docs, SLACK).leaks());
}

#[test]
fn growth_just_past_slack_is_flagged() {
    assert_eq!(
        classify_leak(1_000, 1_000 + SLACK, SLACK),
        LeakVerdict::Bounded
    );
    assert_eq!(
        classify_leak(1_000, 1_000 + SLACK + 1, SLACK),
        LeakVerdict::Leaking,
    );
}

#[test]
fn default_residual_is_zero() {
    assert_eq!(
        ResidualStats::default(),
        ResidualStats {
            curr_bytes: 0,
            curr_blocks: 0
        }
    );
}
