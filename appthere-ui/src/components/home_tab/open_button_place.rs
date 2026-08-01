// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The Open action's two pure decisions: which presentation, and where the
//! tooltip goes (Spec 08 T4.3).
//!
//! Split from `open_button.rs` at the 300-line ceiling. The seam is the one the
//! popover primitive uses throughout — the decisions are testable without a
//! pointer or a runtime, the component only wires them.

use crate::components::popover::{Align, PlacementRequest, Rect, Side, MIN_ANCHORED_HEIGHT_PX};
use crate::device_profile::PointerPrecision;

/// Tooltip width. Narrow: it holds one short phrase, and a tooltip wider than
/// its text reads as a panel.
const TOOLTIP_WIDTH_PX: f32 = 160.0;
/// One line of label text plus its padding.
const TOOLTIP_HEIGHT_PX: f32 = MIN_ANCHORED_HEIGHT_PX;
/// Gap between the button and its tooltip.
const TOOLTIP_GAP_PX: f32 = 6.0;
/// Distance kept from every viewport edge.
const EDGE_MARGIN_PX: f32 = 8.0;

/// A viewport that constrains nothing, for the frame before the window is
/// measured. The host overwrites it.
const UNBOUNDED_UNTIL_MEASURED: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: 1.0e6,
    height: 1.0e6,
};

/// Where the tooltip goes, given the button's rect.
///
/// Below and start-aligned. Below rather than above because this button sits at
/// the *top* of its section, so above is where the window edge is — and `place`
/// flipping back down would be a wasted decision on the common path.
#[must_use]
pub(crate) fn tooltip_placement(anchor: Rect) -> PlacementRequest {
    PlacementRequest {
        anchor,
        width: TOOLTIP_WIDTH_PX,
        height: TOOLTIP_HEIGHT_PX,
        viewport: UNBOUNDED_UNTIL_MEASURED,
        preferred: Side::Below,
        align: Align::Start,
        gap: TOOLTIP_GAP_PX,
        margin: EDGE_MARGIN_PX,
        // One line is the whole tooltip, so the floor is one touch target rather
        // than the two-row menu minimum: there is no "the list continues" to
        // hide, which is the only thing the larger minimum protects.
        min_anchored_height: MIN_ANCHORED_HEIGHT_PX,
    }
}

/// Whether the Open action shows a hover tooltip or a visible label.
///
/// Extracted so the branch is testable without a pointer: it is the decision
/// T4.0 recorded as never having run, since `DeviceProfile::pointer` had no
/// production writer and `has_hover` no production reader.
#[must_use]
pub(crate) fn shows_visible_label(pointer: PointerPrecision) -> bool {
    !pointer.has_hover()
}
