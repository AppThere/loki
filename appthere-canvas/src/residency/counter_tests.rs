// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`super::TextureResidency`]. Extracted per the file-ceiling idiom.
//!
//! # One test, on purpose
//!
//! The counters are process-wide statics and `cargo test` runs tests on
//! parallel threads, so two test functions touching them would interleave and
//! fail intermittently — the flake being *caused by* the harness rather than
//! found by it. Splitting them and reaching for a serialising dependency would
//! buy nicer failure names at the cost of a crate; one sequential test with
//! labelled sections buys the same isolation for free. Any new counter
//! assertion belongs inside this function.

use super::TextureResidency;

#[test]
fn counter_accumulates_peaks_and_balances() {
    TextureResidency::reset();
    assert_eq!(TextureResidency::resident(), 0);
    assert!(TextureResidency::snapshot().is_balanced());

    // — accumulation —
    TextureResidency::record_alloc(1_000);
    TextureResidency::record_alloc(2_500);
    assert_eq!(TextureResidency::resident(), 3_500);
    assert_eq!(TextureResidency::peak(), 3_500);

    // — peak survives a release: the high-water mark is the budget-relevant
    //   figure, and a counter that forgot it would report a comfortable
    //   steady state for a session that had already spiked over budget.
    TextureResidency::record_free(2_500);
    assert_eq!(TextureResidency::resident(), 1_000);
    assert_eq!(TextureResidency::peak(), 3_500);

    // — balance —
    TextureResidency::record_free(1_000);
    let snap = TextureResidency::snapshot();
    assert_eq!(snap.resident, 0);
    assert_eq!(snap.allocs, 2);
    assert_eq!(snap.frees, 2);
    assert!(snap.is_balanced());

    // — an unbalanced release saturates rather than wrapping, and the
    //   allocs/frees halves of `is_balanced` still catch it. A wrapping
    //   counter would read ~16 EB and be dismissed as broken instrumentation.
    TextureResidency::record_free(9_999);
    assert_eq!(TextureResidency::resident(), 0);
    assert!(
        !TextureResidency::snapshot().is_balanced(),
        "an extra release must show up as an imbalance, not vanish"
    );

    // — reset clears every counter, including the peak —
    TextureResidency::reset();
    let snap = TextureResidency::snapshot();
    assert_eq!(snap, Default::default());
    assert!(snap.is_balanced());
}
