// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`super::DialogPosture`] — the pure breakpoint→layout mapping.

use super::*;

/// Compact is the *only* class that becomes a sheet. Asserting the Compact case
/// alone would pass for a posture that was full-screen everywhere, so both
/// pointer classes are asserted false.
#[test]
fn only_compact_goes_full_screen() {
    assert!(DialogPosture::for_breakpoint(Breakpoint::Compact).full_screen);
    assert!(!DialogPosture::for_breakpoint(Breakpoint::Medium).full_screen);
    assert!(!DialogPosture::for_breakpoint(Breakpoint::Expanded).full_screen);
}

/// Note 05: the preview rail docks at Expanded and *only* there — at Medium the
/// form column would lose more width than the preview gains.
#[test]
fn preview_docks_only_at_expanded() {
    assert!(DialogPosture::for_breakpoint(Breakpoint::Expanded).preview_docked);
    assert!(!DialogPosture::for_breakpoint(Breakpoint::Medium).preview_docked);
    assert!(!DialogPosture::for_breakpoint(Breakpoint::Compact).preview_docked);
}

/// WCAG 2.5.8: touch targets are enforced at the touch-first class, and the
/// pointer classes must *not* pay for them (they keep desktop density).
#[test]
fn touch_targets_apply_at_compact_only() {
    let compact = DialogPosture::for_breakpoint(Breakpoint::Compact);
    assert!(compact.min_touch_px >= 44.0, "WCAG 2.5.8");
    assert!(compact.touch_min_css().contains("min-height"));

    for bp in [Breakpoint::Medium, Breakpoint::Expanded] {
        let p = DialogPosture::for_breakpoint(bp);
        assert_eq!(p.min_touch_px, 0.0);
        assert!(
            p.touch_min_css().is_empty(),
            "pointer classes emit no min-height"
        );
    }
}

/// The three width postures are mutually distinguishable: a sheet has no
/// max-width cap, Medium caps below the Expanded fixed width, and Expanded pins
/// the fixed width for the class.
#[test]
fn card_width_differs_across_all_three_classes() {
    let wide = DialogWidth::Wide;
    let compact = DialogPosture::for_breakpoint(Breakpoint::Compact).card_width_css(wide);
    let medium = DialogPosture::for_breakpoint(Breakpoint::Medium).card_width_css(wide);
    let expanded = DialogPosture::for_breakpoint(Breakpoint::Expanded).card_width_css(wide);

    assert!(compact.contains("height: 100%"), "sheet fills the frame");
    assert!(!compact.contains("max-width"), "a sheet has no cap");
    assert!(medium.contains(&format!(
        "max-width: {}px",
        tokens::DIALOG_WIDTH_MEDIUM_MAX_PX
    )));
    assert!(expanded.contains(&format!("width: {}px", tokens::DIALOG_WIDTH_WIDE_PX)));
    assert_ne!(compact, medium);
    assert_ne!(medium, expanded);
}

/// Wide and narrow dialogs differ at Expanded — where the fixed width applies —
/// and deliberately converge at Medium and Compact, which size to the frame.
#[test]
fn width_class_separates_only_where_the_fixed_width_applies() {
    let expanded = DialogPosture::for_breakpoint(Breakpoint::Expanded);
    assert_ne!(
        expanded.card_width_css(DialogWidth::Wide),
        expanded.card_width_css(DialogWidth::Narrow)
    );
    assert!(DialogWidth::Wide.expanded_px() > DialogWidth::Narrow.expanded_px());

    for bp in [Breakpoint::Medium, Breakpoint::Compact] {
        let p = DialogPosture::for_breakpoint(bp);
        assert_eq!(
            p.card_width_css(DialogWidth::Wide),
            p.card_width_css(DialogWidth::Narrow),
            "below Expanded the card sizes to the frame, not the width class"
        );
    }
}

/// A sheet meets the frame edges, so a corner radius would show the backdrop
/// through the corners; a floating card keeps the large-surface radius.
#[test]
fn sheet_is_square_and_card_is_rounded() {
    assert_eq!(
        DialogPosture::for_breakpoint(Breakpoint::Compact).card_radius_px(),
        0.0
    );
    for bp in [Breakpoint::Medium, Breakpoint::Expanded] {
        assert_eq!(
            DialogPosture::for_breakpoint(bp).card_radius_px(),
            tokens::RADIUS_LG
        );
    }
}

/// Design note 06: Compact footer actions stack full-width, primary first.
#[test]
fn footer_stacks_only_at_compact() {
    assert_eq!(
        DialogPosture::for_breakpoint(Breakpoint::Compact).footer_direction(),
        "column"
    );
    for bp in [Breakpoint::Medium, Breakpoint::Expanded] {
        assert_eq!(DialogPosture::for_breakpoint(bp).footer_direction(), "row");
    }
}
