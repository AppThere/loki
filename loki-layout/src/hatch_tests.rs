// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use crate::color::LayoutColor;
use crate::geometry::LayoutRect;
use crate::hatch::{HatchPattern, PositionedHatch};

fn hatch(pattern: HatchPattern, thin: bool) -> PositionedHatch {
    PositionedHatch {
        rect: LayoutRect::new(10.0, 20.0, 40.0, 30.0),
        fill: None,
        color: LayoutColor::BLACK,
        pattern,
        thin,
    }
}

#[test]
fn horizontal_lines_span_full_width_inside_the_rect() {
    let segs = hatch(HatchPattern::Horizontal, false).segments();
    assert!(!segs.is_empty(), "expected horizontal hatch lines");
    for s in &segs {
        assert!((s.x0 - 10.0).abs() < 1e-3, "starts at left edge");
        assert!((s.x1 - 50.0).abs() < 1e-3, "ends at right edge");
        assert!((s.y0 - s.y1).abs() < 1e-3, "line is horizontal");
        assert!(s.y0 > 20.0 && s.y0 < 50.0, "within the rect vertically");
    }
}

#[test]
fn vertical_lines_span_full_height_inside_the_rect() {
    let segs = hatch(HatchPattern::Vertical, false).segments();
    assert!(!segs.is_empty());
    for s in &segs {
        assert!((s.x0 - s.x1).abs() < 1e-3, "line is vertical");
        assert!((s.y0 - 20.0).abs() < 1e-3 && (s.y1 - 50.0).abs() < 1e-3);
        assert!(s.x0 > 10.0 && s.x0 < 50.0, "within the rect horizontally");
    }
}

#[test]
fn cross_is_horizontal_plus_vertical() {
    let h = hatch(HatchPattern::Horizontal, false).segments().len();
    let v = hatch(HatchPattern::Vertical, false).segments().len();
    let cross = hatch(HatchPattern::Cross, false).segments().len();
    assert_eq!(cross, h + v);
}

#[test]
fn diagonals_stay_within_the_rect_and_are_non_empty() {
    for p in [
        HatchPattern::DiagUp,
        HatchPattern::DiagDown,
        HatchPattern::DiagCross,
    ] {
        let segs = hatch(p, false).segments();
        assert!(!segs.is_empty(), "expected diagonal hatch lines for {p:?}");
        for s in &segs {
            for (x, y) in [(s.x0, s.y0), (s.x1, s.y1)] {
                assert!((10.0 - 1e-3..=50.0 + 1e-3).contains(&x), "x {x} in rect");
                assert!((20.0 - 1e-3..=50.0 + 1e-3).contains(&y), "y {y} in rect");
            }
        }
    }
}

#[test]
fn thin_lines_are_thinner_and_closer() {
    let normal = hatch(HatchPattern::Horizontal, false);
    let thin = hatch(HatchPattern::Horizontal, true);
    assert!(thin.line_width() < normal.line_width());
    assert!(thin.spacing() < normal.spacing());
    // Closer spacing → more lines.
    assert!(thin.segments().len() > normal.segments().len());
}

#[test]
fn zero_area_rect_has_no_segments() {
    let mut h = hatch(HatchPattern::Cross, false);
    h.rect = LayoutRect::new(0.0, 0.0, 0.0, 0.0);
    assert!(h.segments().is_empty());
}
