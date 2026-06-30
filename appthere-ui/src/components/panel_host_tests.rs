// SPDX-License-Identifier: Apache-2.0

//! `AtPanelHost` posture (Spec 05 M3 / ADR-0013): the breakpoint-driven posture
//! is a pure decision, testable without a real window (Spec 03 D1).

use super::*;
use crate::tokens::layout::PANEL_SIDE_WIDTH_PX;

/// Resolve every size class through the posture decision (a runtime call, so the
/// assertions below are not constant-folded).
fn posture(bp: Breakpoint) -> PanelPosture {
    PanelPosture::for_breakpoint(bp)
}

#[test]
fn compact_is_a_full_width_sheet() {
    // Compact has no room beside the document → full-width sheet.
    assert_eq!(posture(Breakpoint::Compact).css_width(), "100%");
}

#[test]
fn medium_and_expanded_are_bounded_side_panels() {
    let expected = format!("{PANEL_SIDE_WIDTH_PX}px");
    assert_eq!(posture(Breakpoint::Medium).css_width(), expected);
    assert_eq!(posture(Breakpoint::Expanded).css_width(), expected);
    assert_eq!(posture(Breakpoint::Expanded).width_px, PANEL_SIDE_WIDTH_PX);
}

#[test]
fn posture_changes_only_on_the_compact_boundary() {
    // Medium and Expanded share the bounded posture; only Compact differs — the
    // host re-lays-out on the class boundary, not per pixel.
    assert_eq!(posture(Breakpoint::Medium), posture(Breakpoint::Expanded));
    assert_ne!(posture(Breakpoint::Compact), posture(Breakpoint::Expanded));
}
