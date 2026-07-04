// SPDX-License-Identifier: Apache-2.0

//! Style-panel posture (Spec 05 M7 / §11): the breakpoint → layout mapping,
//! verified without a window (Spec 03 D1).

use super::*;
use appthere_ui::responsive::Breakpoint;

#[test]
fn compact_stacks_full_width_with_touch_targets() {
    let p = StylePanelPosture::for_breakpoint(Breakpoint::Compact);
    assert!(p.stack, "Compact stacks the body vertically");
    assert!(p.full_width, "Compact sections fill the width");
    assert_eq!(p.body_direction(), "column");
    assert_eq!(p.section_width(160.0), "100%");
    assert!(
        p.min_touch_px >= 44.0,
        "Compact meets the 44px touch minimum"
    );
    assert!(p.touch_min_css().contains("min-height"));
    assert!(
        p.height_px > super::super::STYLE_EDITOR_HEIGHT_PX,
        "Compact sheet is taller than the Expanded side panel"
    );
}

#[test]
fn medium_and_expanded_keep_side_by_side_columns() {
    for bp in [Breakpoint::Medium, Breakpoint::Expanded] {
        let p = StylePanelPosture::for_breakpoint(bp);
        assert!(!p.stack, "{bp:?} keeps side-by-side columns");
        assert!(!p.full_width);
        assert_eq!(p.body_direction(), "row");
        assert_eq!(p.section_width(220.0), "220px");
        assert_eq!(p.min_touch_px, 0.0, "{bp:?} uses natural control heights");
        assert!(p.touch_min_css().is_empty());
        assert_eq!(p.height_px, super::super::STYLE_EDITOR_HEIGHT_PX);
    }
}
