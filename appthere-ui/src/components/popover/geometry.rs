// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Where an overlay goes (Spec 08 T4.1).
//!
//! # Pure, because this is where the bugs are
//!
//! Phase 4 offers few natural counters, and this program's record is four
//! retracted timing attributions against no wrong counter. Placement, collision
//! flip, shift-into-viewport and anchor arithmetic are all *computable*, so they
//! are computed here and asserted exactly — the move that settled R28. What is
//! left for a screen is then genuinely small: does it look right, and does
//! dismissal feel right.
//!
//! # What the existing popup did, and why it is not the reference
//!
//! `editor_spell_panel` clamped horizontally against the viewport width and took
//! `anchor_y.max(0.0)` vertically. It never received viewport *height*, so a menu
//! opened near the bottom of the screen ran off it — measured at **298px of a
//! 320px menu below the fold**. That case was written as a failing test against
//! the old behaviour before this was implemented, and is preserved as
//! `an_overlay_near_the_bottom_edge_stays_inside_the_viewport`.
//!
//! # Scope: four consumers, deliberately
//!
//! T4.2's Recent Documents menu and T5.2's colour picker anchor to small
//! controls; T5.4's zoom popover carries a preset list and a field and anchors to
//! the status bar; T7.1's overflow anchors to a bar that sits against an edge. So
//! the API takes an **anchor rect** rather than a point, an **alignment** so an
//! overlay can be right-aligned to a control near the right edge, and reports a
//! **clamped size** so a list can scroll rather than overflow.
//!
//! **`Before`/`After` (horizontal placement) is deliberately absent.** All four
//! known consumers place vertically. Adding a horizontal axis now would be
//! designing for a consumer nobody has named, and the flip/shift logic doubles
//! with it. When a submenu or a horizontally-anchored tooltip appears, the axis
//! is added then — with a consumer to check it against.

/// An axis-aligned rectangle in CSS pixels.
///
/// Local rather than `loki_primitives::Rect`, which is generic over units and
/// carries `Length<U>`: overlay geometry is CSS pixels and nothing else, and
/// `appthere-ui` does not otherwise depend on the document model.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Rect {
    /// A rectangle at `(x, y)` of `width` × `height`.
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Right edge.
    #[must_use]
    pub fn right(self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge.
    #[must_use]
    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    /// Whether `self` lies wholly within `outer`.
    #[must_use]
    pub fn is_inside(self, outer: Rect) -> bool {
        self.x >= outer.x
            && self.y >= outer.y
            && self.right() <= outer.right()
            && self.bottom() <= outer.bottom()
    }

    /// Whether `self` and `other` share any area.
    ///
    /// Used to assert an overlay never covers its own anchor — the failure a
    /// reader sees when a flip is computed with the wrong sign, and one that
    /// plausible-looking offsets will not reveal.
    #[must_use]
    pub fn overlaps(self, other: Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }
}

/// Which side of the anchor the overlay sits on.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Side {
    /// Above the anchor — the status bar's direction.
    Above,
    /// Below the anchor — the default for menus and pickers.
    #[default]
    Below,
}

impl Side {
    /// The other side.
    #[must_use]
    pub fn flipped(self) -> Self {
        match self {
            Self::Above => Self::Below,
            Self::Below => Self::Above,
        }
    }
}

/// How the overlay lines up with the anchor across the placement axis.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Align {
    /// Left edges together — a menu dropping from a caret.
    #[default]
    Start,
    /// Centres together — a picker under a swatch.
    Center,
    /// Right edges together — an overflow menu under a control near the right
    /// edge, where `Start` would push it off.
    End,
}

/// What to place, and in what space. All values in CSS pixels.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PlacementRequest {
    /// The control or caret the overlay belongs to. A caret is a zero-width rect.
    pub anchor: Rect,
    /// Desired overlay width.
    pub width: f32,
    /// Desired overlay height. May be reduced — see [`Placement::clamped`].
    pub height: f32,
    /// The space the overlay must stay inside.
    ///
    /// Not necessarily the window: a caller with a soft keyboard or a safe-area
    /// inset passes the *usable* rect, so T3.2's keyboard avoidance composes with
    /// this rather than fighting it.
    pub viewport: Rect,
    /// Preferred side.
    pub preferred: Side,
    /// Alignment across the placement axis.
    pub align: Align,
    /// Gap between the anchor and the overlay.
    pub gap: f32,
    /// Minimum distance kept from every viewport edge.
    pub margin: f32,
}

/// The resolved position, with what had to be conceded to reach it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Placement {
    /// Final overlay rectangle. Inside the viewport whenever anything fits.
    pub rect: Rect,
    /// The side actually used.
    pub side: Side,
    /// The preferred side did not fit and the other was used.
    pub flipped: bool,
    /// The aligned position would have crossed a viewport edge and was moved.
    pub shifted: bool,
    /// The requested size did not fit and was reduced — the content must scroll.
    pub clamped: bool,
}

#[path = "geometry_place.rs"]
mod place_impl;
pub use place_impl::place;

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod tests;
