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
///
/// # Edge conventions, because there are three of them
///
/// | predicate | axes | edges | degenerate rects |
/// | --- | --- | --- | --- |
/// | [`Rect::is_inside`] | both | **closed** — flush is inside | a zero-size rect on the edge is inside |
/// | [`Rect::intersects_closed`] | both | **closed** — touching counts | a caret on the edge intersects |
/// | [`Rect::covers_vertically_open`] | y only | **open** — flush is not covering | an empty rect covers nothing |
/// | `wiring::contains` (point) | both | **closed** | a zero-size rect contains its own corner |
///
/// The conventions genuinely differ — a caret flush with the viewport edge must
/// keep its menu (closed), while a menu resting flush against that caret is not
/// on top of it (open) — so the divergence is stated once here and carried in the
/// method names rather than left to be discovered. A name that does not say
/// which convention it uses is the L08-031 shape: the next author picks whichever
/// reads right and inherits the edge behaviour they did not ask about.
///
/// **A strict test collapses on degenerate rects**, which is not a corner case
/// here: a caret is zero-width *by nature*, and `a < b.right()` reading
/// `400.0 < 400.0` is how `overlaps` — the predicate these replaced — reported
/// "no collision" for a menu drawn straight over its caret.
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

    /// Whether `self` lies wholly within `outer`. **Edges closed**: a rect flush
    /// with a viewport edge is inside it.
    ///
    /// See the type's docs for the table of edge conventions — this crate has
    /// three of them and the names now say which.
    #[must_use]
    pub fn is_inside(self, outer: Rect) -> bool {
        self.x >= outer.x
            && self.y >= outer.y
            && self.right() <= outer.right()
            && self.bottom() <= outer.bottom()
    }

    /// Whether `self` and `other` share at least a point, **edges closed** —
    /// touching counts.
    ///
    /// The convention is in the name because this crate also has an *open* one
    /// two methods down, and the difference decides whether a caret sitting
    /// exactly on the viewport edge keeps its menu open. `_closed` is what
    /// [`super::interaction::anchor_is_anchorable`] needs: a caret is a
    /// zero-width rect, so an open test reports every caret on the left margin
    /// as gone.
    #[must_use]
    pub fn intersects_closed(self, other: Rect) -> bool {
        self.right() >= other.x
            && other.right() >= self.x
            && self.bottom() >= other.y
            && other.bottom() >= self.y
    }

    /// Whether `self` sits over `other` **on the placement axis**, **edges
    /// open** — resting flush against the anchor is not covering it — the
    /// failure a reader sees when a flip is computed with the wrong sign, and one
    /// that plausible-looking offsets will not reveal.
    ///
    /// # Why this replaced a plain rectangle intersection
    ///
    /// The obvious predicate — "do the two rects share any area" — is
    /// **vacuously false for the anchor shape the first consumer uses**. A caret
    /// is a zero-width rect and `Align::Start` puts the overlay's left edge on
    /// it, so `overlay.x < caret.right()` is `400.0 < 400.0`: false, whatever the
    /// vertical arithmetic did.
    ///
    /// The mechanism is **flush contact, not degeneracy on its own** — a
    /// zero-width rect strictly inside another does register under an open test
    /// (checked in `loki_primitives::Rect`'s suite, which pins both halves).
    /// Degeneracy matters because a caret is *entirely* boundary, and alignment
    /// is what puts that boundary on an edge. Which is to say: in laid-out UI
    /// geometry the collapsing case is the ordinary one. `the_menu_never_covers_the_caret_it_belongs_to`
    /// was written with it and **could not fail** — a menu placed deliberately
    /// on top of the caret reported no overlap. Checked, not inferred.
    ///
    /// Two separate causes, both fixed here:
    ///
    /// 1. **The horizontal test does not belong in this question.** An overlay is
    ///    *aligned* to its anchor, so sharing its x range is the design, not the
    ///    defect. Including x turns a real assertion into a coin flip on the
    ///    alignment.
    /// 2. **Degenerate rects cover nothing.** A zero-height overlay is not on
    ///    screen, so it obscures nothing — and a strict interval test says
    ///    otherwise for a rect *contained* in another. Handled here rather than
    ///    as an `if` in each test, so no caller has to remember it.
    ///
    /// # PRECONDITION: placement is vertical
    ///
    /// `Side` has only `Above`/`Below`, so an overlay sharing the anchor's
    /// vertical band **is** on top of it. Should `Before`/`After` ever be added
    /// (see the module docs — deliberately absent), this becomes wrong in the
    /// direction that reports a correct side-by-side placement as a collision,
    /// and it must gain the horizontal case at the same time.
    #[must_use]
    pub fn covers_vertically_open(self, other: Rect) -> bool {
        self.height > 0.0
            && other.height > 0.0
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
    /// Below this height the **anchored** form is not worth showing and
    /// [`super::presentation::present`] falls back to a modal.
    ///
    /// # Why the consumer states it and the primitive decides
    ///
    /// The consumer knows its content — a menu of four actions, a preset list, a
    /// colour picker whose SV square has a minimum of its own — and the
    /// primitive knows the geometry. Splitting it that way keeps the "wires,
    /// does not decide" invariant intact in both directions: no consumer picks
    /// its own anchored-vs-modal rule, and the primitive does not pretend to know
    /// how tall a picker needs to be.
    ///
    /// [`place`] ignores this field. It lives on the request rather than in
    /// `present`'s signature because `on_anchor_change` needs the same value to
    /// judge a mid-life change, and two paths carrying it separately is exactly
    /// how they drift (L08-028).
    ///
    /// **A floor applies regardless**: `present` raises anything below
    /// [`super::presentation::MIN_ANCHORED_HEIGHT_PX`] to it, so passing `0.0`
    /// cannot produce a menu shorter than one touch target. Menus should pass
    /// [`super::presentation::MIN_ANCHORED_MENU_PX`].
    pub min_anchored_height: f32,
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
