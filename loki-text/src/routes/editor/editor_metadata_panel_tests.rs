// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the metadata-panel responsive label-stacking helper (Spec 03
//! R-13g). The label stacks above its input only on very narrow viewports so
//! the input keeps a usable width.

use super::{METADATA_LABEL_STACK_PX, stack_labels};

#[test]
fn wide_viewport_keeps_side_by_side_labels() {
    assert!(!stack_labels(1280.0));
    assert!(!stack_labels(600.0));
    // Exactly at the threshold is still side-by-side (strict `<`).
    assert!(!stack_labels(METADATA_LABEL_STACK_PX));
}

#[test]
fn narrow_viewport_stacks_labels() {
    assert!(stack_labels(249.0));
    assert!(stack_labels(200.0));
    assert!(stack_labels(120.0));
}

#[test]
fn unmeasured_viewport_keeps_the_wide_layout() {
    // 0.0 (or negative) means "not measured yet" — do not stack; the first
    // measured frame corrects it.
    assert!(!stack_labels(0.0));
    assert!(!stack_labels(-5.0));
}
