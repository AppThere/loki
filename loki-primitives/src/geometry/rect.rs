// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::{insets::Insets, point::Point, size::Size};
use crate::units::Length;

/// An axis-aligned rectangle defined by an origin and size.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rect<U> {
    /// Origin of rect
    pub origin: Point<U>,
    /// Size geometry
    pub size: Size<U>,
}

impl<U: Copy> Rect<U> {
    /// Factory for creating rect via size/origin definitions.
    #[must_use]
    pub fn new(origin: Point<U>, size: Size<U>) -> Self {
        Self { origin, size }
    }

    /// Factory method spanning left right top bottoms limits.
    #[must_use]
    pub fn from_ltrb(left: Length<U>, top: Length<U>, right: Length<U>, bottom: Length<U>) -> Self {
        Self::new(Point::new(left, top), Size::new(right - left, bottom - top))
    }

    /// Returns point min
    #[must_use]
    pub fn min_x(self) -> Length<U> {
        self.origin.x
    }

    /// Returns point min
    #[must_use]
    pub fn min_y(self) -> Length<U> {
        self.origin.y
    }

    /// Returns point max
    #[must_use]
    pub fn max_x(self) -> Length<U> {
        self.origin.x + self.size.width
    }

    /// Returns point max
    #[must_use]
    pub fn max_y(self) -> Length<U> {
        self.origin.y + self.size.height
    }

    /// Produces its central coordinates
    #[must_use]
    pub fn center(self) -> Point<U> {
        Point::new(
            self.origin.x + (self.size.width / 2.0),
            self.origin.y + (self.size.height / 2.0),
        )
    }

    /// Detects inclusion
    #[must_use]
    pub fn contains_point(self, p: Point<U>) -> bool {
        p.x.value() >= self.min_x().value()
            && p.x.value() < self.max_x().value()
            && p.y.value() >= self.min_y().value()
            && p.y.value() < self.max_y().value()
    }

    /// Tests for crossing. **Edges open**: rects that merely touch do not
    /// intersect.
    ///
    /// # The hazard is flush contact, and degenerate rects are all contact
    ///
    /// Stated precisely because the loose version — "an open test is false for
    /// any degenerate rect" — is not true, and the test below is what settled
    /// it: a zero-width rect *strictly inside* another does intersect it. What
    /// an open test rejects is **touching**, and a degenerate rect is entirely
    /// boundary, so it is rejected exactly when it lies flush with an edge.
    ///
    /// That distinction decides whether the trap fires, because UI geometry
    /// *aligns* things: put a caret on a page's left margin, or start a menu at
    /// the caret's x, and flush contact is the normal case rather than an edge
    /// case. That is how it produced a live defect one crate over — a popover's
    /// "does the menu cover its own caret" assertion used an open intersection
    /// against a zero-width, left-aligned caret, so `menu.x < caret.right()`
    /// read `400.0 < 400.0` and the test could not fail.
    ///
    /// # Stated because it has no caller yet, which is when a trap is cheapest
    /// to label
    ///
    /// The workspace has four `Rect` types with three different edge
    /// conventions: this one is open, `loki_layout::LayoutRect::intersects` is
    /// closed (its docs say so), and `contains_point` is half-open here and
    /// closed in `loki-layout` and `loki-graphics`. None of the names disclose
    /// it. Whatever first calls this should decide, deliberately, whether a
    /// collapsed selection, an empty line box or a hairline rule flush with a
    /// boundary must count.
    ///
    /// [`Self::intersection`] inherits the convention.
    #[must_use]
    pub fn intersects(self, other: Self) -> bool {
        self.min_x().value() < other.max_x().value()
            && self.max_x().value() > other.min_x().value()
            && self.min_y().value() < other.max_y().value()
            && self.max_y().value() > other.min_y().value()
    }

    /// Merging overlapping segments or returning nothing
    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        if !self.intersects(other) {
            return None;
        }
        let min_x = self.min_x().max(other.min_x());
        let min_y = self.min_y().max(other.min_y());
        let max_x = self.max_x().min(other.max_x());
        let max_y = self.max_y().min(other.max_y());
        Some(Self::from_ltrb(min_x, min_y, max_x, max_y))
    }

    /// Creates bounding rect for both rects.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        let min_x = self.min_x().min(other.min_x());
        let min_y = self.min_y().min(other.min_y());
        let max_x = self.max_x().max(other.max_x());
        let max_y = self.max_y().max(other.max_y());
        Self::from_ltrb(min_x, min_y, max_x, max_y)
    }

    /// Insets (shrinks) rect
    #[must_use]
    pub fn inset(self, insets: Insets<U>) -> Self {
        Self::from_ltrb(
            self.min_x() + insets.left,
            self.min_y() + insets.top,
            self.max_x() - insets.right,
            self.max_y() - insets.bottom,
        )
    }

    /// Outsets (expands) rect
    #[must_use]
    pub fn outset(self, insets: Insets<U>) -> Self {
        Self::from_ltrb(
            self.min_x() - insets.left,
            self.min_y() - insets.top,
            self.max_x() + insets.right,
            self.max_y() + insets.bottom,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Pt;
    use approx::assert_relative_eq;

    #[test]
    fn test_rect_intersection() {
        let r1 = Rect::<Pt>::from_ltrb(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(10.0),
            Length::new(10.0),
        );
        let r2 = Rect::<Pt>::from_ltrb(
            Length::new(5.0),
            Length::new(5.0),
            Length::new(15.0),
            Length::new(15.0),
        );

        let int = r1.intersection(r2).unwrap();
        assert_relative_eq!(int.min_x().value(), 5.0);
        assert_relative_eq!(int.min_y().value(), 5.0);
        assert_relative_eq!(int.max_x().value(), 10.0);
        assert_relative_eq!(int.max_y().value(), 10.0);

        let disjoint = Rect::<Pt>::from_ltrb(
            Length::new(20.0),
            Length::new(20.0),
            Length::new(30.0),
            Length::new(30.0),
        );
        assert!(r1.intersection(disjoint).is_none());
    }

    /// The open convention pinned where it bites, and where it does not.
    ///
    /// Written to check the loose claim that an open test is false for *any*
    /// degenerate rect; it is not, and knowing which half is true is what tells
    /// a future caller whether it is exposed. Both halves are asserted so the
    /// distinction cannot be lost to a later edit.
    ///
    /// Not a fix — nothing calls `intersects` yet, and changing a shared
    /// primitive's semantics for a hypothetical caller is worse than labelling
    /// it. This exists so the first caller meets the convention as a test it can
    /// read rather than as a defect it has to find, which is the order it went
    /// in one crate over.
    #[test]
    fn a_degenerate_rect_intersects_unless_it_lies_flush_with_an_edge() {
        let page = Rect::<Pt>::from_ltrb(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(10.0),
            Length::new(10.0),
        );
        let caret_inside = Rect::<Pt>::from_ltrb(
            Length::new(5.0),
            Length::new(2.0),
            Length::new(5.0),
            Length::new(8.0),
        );
        assert!(
            page.intersects(caret_inside),
            "a zero-width caret strictly inside does intersect — the collapse is \
             not degeneracy on its own",
        );
        let caret_on_the_margin = Rect::<Pt>::from_ltrb(
            Length::new(0.0),
            Length::new(2.0),
            Length::new(0.0),
            Length::new(8.0),
        );
        assert!(
            !page.intersects(caret_on_the_margin),
            "a caret flush with the left margin is reported as outside the page \
             it sits in — this is the case that bites, and UI geometry aligns \
             things to edges as a matter of course",
        );
        assert!(page.intersection(caret_on_the_margin).is_none());
    }

    #[test]
    fn test_rect_union() {
        let r1 = Rect::<Pt>::from_ltrb(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(10.0),
            Length::new(10.0),
        );
        let r2 = Rect::<Pt>::from_ltrb(
            Length::new(5.0),
            Length::new(5.0),
            Length::new(15.0),
            Length::new(15.0),
        );

        let u = r1.union(r2);
        assert_relative_eq!(u.min_x().value(), 0.0);
        assert_relative_eq!(u.min_y().value(), 0.0);
        assert_relative_eq!(u.max_x().value(), 15.0);
        assert_relative_eq!(u.max_y().value(), 15.0);
    }

    #[test]
    fn test_rect_inset() {
        let r1 = Rect::<Pt>::from_ltrb(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(10.0),
            Length::new(10.0),
        );
        let insets = Insets::<Pt>::new(
            Length::new(1.0),
            Length::new(2.0),
            Length::new(3.0),
            Length::new(4.0),
        );
        let shrunk = r1.inset(insets);
        assert_relative_eq!(shrunk.min_x().value(), 4.0);
        assert_relative_eq!(shrunk.min_y().value(), 1.0);
        assert_relative_eq!(shrunk.max_x().value(), 8.0);
        assert_relative_eq!(shrunk.max_y().value(), 7.0);
    }

    #[test]
    fn test_contains_point() {
        let r = Rect::<Pt>::from_ltrb(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(10.0),
            Length::new(10.0),
        );
        assert!(r.contains_point(Point::new(Length::new(5.0), Length::new(5.0))));
        assert!(r.contains_point(Point::new(Length::new(0.0), Length::new(0.0)))); // corner inclusive minimum
        assert!(!r.contains_point(Point::new(Length::new(10.0), Length::new(10.0)))); // corner exclusive bound
        assert!(!r.contains_point(Point::new(Length::new(15.0), Length::new(5.0))));
    }
}
