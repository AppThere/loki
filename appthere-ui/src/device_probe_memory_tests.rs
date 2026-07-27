// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the memory resampling policy. The Dioxus-side loop needs a runtime
//! and is not covered here; what *is* covered is the materiality rule, which is
//! the part with a failure mode.

use super::{quantise_bytes, quantised, MEMORY_QUANTUM_BYTES};
use crate::device_probe::SystemMemory;

const GIB: u64 = 1024 * 1024 * 1024;

/// The property the whole design turns on: jitter inside a bucket is invisible.
#[test]
fn small_fluctuations_land_in_the_same_bucket() {
    let base = 8 * GIB;
    let a = quantise_bytes(base);
    for delta in [1_u64, 1024, 16 * 1024 * 1024, 100 * 1024 * 1024] {
        assert_eq!(quantise_bytes(base + delta), a, "+{delta} B changed bucket");
        assert_eq!(quantise_bytes(base - delta), a, "-{delta} B changed bucket");
    }
}

/// The failure relative hysteresis would have had, asserted directly rather than
/// argued: a machine drifting down in small steps must eventually register.
///
/// With a 12.5%-of-stored threshold, each of these steps is below it and the
/// figure would never move — leaving a budget derived from 8 GiB on a machine
/// with 2 GiB left, which is the exact case the derivation exists for.
#[test]
fn slow_drift_is_not_ratcheted_away() {
    let mut seen = quantise_bytes(8 * GIB);
    let mut moves = 0;
    let mut available = 8 * GIB;
    // 6% steps: comfortably under any plausible relative threshold.
    while available > 2 * GIB {
        available -= available / 16;
        let q = quantise_bytes(available);
        if q != seen {
            moves += 1;
            seen = q;
        }
    }
    assert!(
        moves >= 4,
        "an 8 GiB -> 2 GiB drift in 6% steps registered only {moves} times; the \
         materiality rule has a ratchet",
    );
    assert_eq!(
        seen,
        quantise_bytes(available),
        "the final bucket must match the final reading",
    );
}

/// A crossing must be reported, and reported once — the point of the grid is
/// that it moves on real change, not that it is quiet.
#[test]
fn crossing_a_bucket_boundary_registers() {
    let lo = quantise_bytes(4 * GIB);
    let hi = quantise_bytes(4 * GIB + MEMORY_QUANTUM_BYTES);
    assert_ne!(lo, hi);
    assert_eq!(hi - lo, MEMORY_QUANTUM_BYTES);
}

/// A machine down to its last few MiB is where the budget matters most, so the
/// figure must not round to zero and read as "never probed" downstream.
#[test]
fn a_nearly_exhausted_machine_does_not_round_to_absent() {
    for tiny in [1_u64, 1024, 4 * 1024 * 1024, 64 * 1024 * 1024] {
        assert!(
            quantise_bytes(tiny) > 0,
            "{tiny} B rounded to zero, which a consumer reads as unprobed",
        );
    }
}

/// An unprobed field stays unprobed. Quantising `None` into `Some(0)` would
/// invent a measurement, which is the L9-009 distinction between "we have not
/// looked" and "we looked and there is nothing".
#[test]
fn absent_figures_stay_absent() {
    let q = quantised(SystemMemory {
        total_bytes: None,
        available_bytes: Some(8 * GIB),
    });
    assert_eq!(q.total_bytes, None);
    assert_eq!(q.available_bytes, Some(quantise_bytes(8 * GIB)));

    let q = quantised(SystemMemory::default());
    assert!(q.is_unknown(), "a wholly unknown probe must stay unknown");
}
