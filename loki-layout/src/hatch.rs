// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Hatch-line geometry for [`PositionedHatch`](crate::hatch::PositionedHatch).
//!
//! A `w:shd` line/cross texture is drawn as a set of thin parallel lines (plus a
//! perpendicular family for the cross variants). This module turns a hatch
//! rect and pattern into rect-clipped line segments in layout space (y-down), so
//! each renderer only has to draw thin filled quads — no clip-state juggling and
//! no renderer-specific geometry.

use crate::color::LayoutColor;
use crate::geometry::LayoutRect;

/// A hatch-shaded rectangle: an optional background fill overlaid with hatch
/// lines in [`color`](Self::color). Emitted for a `w:shd` line/cross texture so
/// the renderer draws the actual lines rather than a flattened tint.
#[derive(Debug, Clone)]
pub struct PositionedHatch {
    /// Position and dimensions.
    pub rect: LayoutRect,
    /// Background fill drawn behind the hatch, or `None` to leave the surface.
    pub fill: Option<LayoutColor>,
    /// Hatch line colour.
    pub color: LayoutColor,
    /// Hatch geometry.
    pub pattern: HatchPattern,
    /// `true` for the `thin*` variants — closer, thinner lines.
    pub thin: bool,
}

/// The geometry of a hatch pattern (renderer-agnostic mirror of the doc-model
/// `HatchPattern`; `loki-layout` keeps its own so [`PositionedItem`](crate::items::PositionedItem)
/// stays free of document-model types, matching the
/// [`BorderStyle`](crate::items::BorderStyle) precedent).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HatchPattern {
    /// Horizontal lines.
    Horizontal,
    /// Vertical lines.
    Vertical,
    /// `/` diagonal lines.
    DiagUp,
    /// `\` diagonal lines.
    DiagDown,
    /// Horizontal + vertical grid.
    Cross,
    /// Both diagonals (an `X` grid).
    DiagCross,
}

/// A clipped hatch line segment in layout coordinates (y increases downward).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HatchSegment {
    /// Start x.
    pub x0: f32,
    /// Start y.
    pub y0: f32,
    /// End x.
    pub x1: f32,
    /// End y.
    pub y1: f32,
}

impl PositionedHatch {
    /// Stroke width of each hatch line in points.
    #[must_use]
    pub fn line_width(&self) -> f32 {
        if self.thin { 0.5 } else { 0.85 }
    }

    /// Perpendicular spacing between adjacent hatch lines in points.
    #[must_use]
    pub fn spacing(&self) -> f32 {
        if self.thin { 3.5 } else { 6.0 }
    }

    /// The hatch lines as rect-clipped segments in layout space. Empty when the
    /// rect has no area.
    #[must_use]
    pub fn segments(&self) -> Vec<HatchSegment> {
        let r = &self.rect;
        let (x0, y0) = (r.origin.x, r.origin.y);
        let (w, h) = (r.size.width, r.size.height);
        if w <= 0.0 || h <= 0.0 {
            return Vec::new();
        }
        let s = self.spacing().max(0.5);
        let mut out = Vec::new();
        let mut push = |seg: Option<HatchSegment>| {
            if let Some(seg) = seg {
                out.push(seg);
            }
        };
        match self.pattern {
            HatchPattern::Horizontal => horizontal(x0, y0, w, h, s, &mut push),
            HatchPattern::Vertical => vertical(x0, y0, w, h, s, &mut push),
            HatchPattern::Cross => {
                horizontal(x0, y0, w, h, s, &mut push);
                vertical(x0, y0, w, h, s, &mut push);
            }
            HatchPattern::DiagUp => diagonal(x0, y0, w, h, s, true, &mut push),
            HatchPattern::DiagDown => diagonal(x0, y0, w, h, s, false, &mut push),
            HatchPattern::DiagCross => {
                diagonal(x0, y0, w, h, s, true, &mut push);
                diagonal(x0, y0, w, h, s, false, &mut push);
            }
        }
        out
    }
}

fn horizontal(
    x0: f32,
    y0: f32,
    w: f32,
    h: f32,
    s: f32,
    push: &mut impl FnMut(Option<HatchSegment>),
) {
    let mut y = y0 + s;
    while y < y0 + h {
        push(Some(HatchSegment {
            x0,
            y0: y,
            x1: x0 + w,
            y1: y,
        }));
        y += s;
    }
}

fn vertical(x0: f32, y0: f32, w: f32, h: f32, s: f32, push: &mut impl FnMut(Option<HatchSegment>)) {
    let mut x = x0 + s;
    while x < x0 + w {
        push(Some(HatchSegment {
            x0: x,
            y0,
            x1: x,
            y1: y0 + h,
        }));
        x += s;
    }
}

/// Emit clipped diagonal lines. `up` = `/` (slope such that x+y is constant),
/// else `\` (y−x constant). Perpendicular spacing is `s`, so the family step
/// along the diagonal invariant is `s·√2`.
fn diagonal(
    x0: f32,
    y0: f32,
    w: f32,
    h: f32,
    s: f32,
    up: bool,
    push: &mut impl FnMut(Option<HatchSegment>),
) {
    let step = s * std::f32::consts::SQRT_2;
    let (x1, y1) = (x0 + w, y0 + h);
    if up {
        // Lines x + y = c crossing the rect; c ∈ [x0+y0, x1+y1].
        let (lo, hi) = (x0 + y0, x1 + y1);
        let mut c = lo + step;
        while c < hi {
            // Direction (1, -1): a long segment, then clip to the rect.
            push(clip(c - y0 - h, y1, c - y0, y0, x0, y0, x1, y1));
            c += step;
        }
    } else {
        // Lines y − x = c crossing the rect; c ∈ [y0−x1, y1−x0].
        let (lo, hi) = (y0 - x1, y1 - x0);
        let mut c = lo + step;
        while c < hi {
            // Direction (1, 1): a long segment, then clip to the rect.
            push(clip(x0, x0 + c, x1, x1 + c, x0, y0, x1, y1));
            c += step;
        }
    }
}

/// Liang–Barsky clip of segment `(ax,ay)-(bx,by)` to the rect
/// `[rx0,rx1]×[ry0,ry1]`. Returns the clipped segment, or `None` if it misses.
#[allow(clippy::too_many_arguments)]
fn clip(
    ax: f32,
    ay: f32,
    bx: f32,
    by: f32,
    rx0: f32,
    ry0: f32,
    rx1: f32,
    ry1: f32,
) -> Option<HatchSegment> {
    let dx = bx - ax;
    let dy = by - ay;
    let mut t0 = 0.0_f32;
    let mut t1 = 1.0_f32;
    let edges = [
        (-dx, ax - rx0),
        (dx, rx1 - ax),
        (-dy, ay - ry0),
        (dy, ry1 - ay),
    ];
    for (p, q) in edges {
        if p == 0.0 {
            if q < 0.0 {
                return None; // parallel and outside
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                if t > t1 {
                    return None;
                }
                if t > t0 {
                    t0 = t;
                }
            } else {
                if t < t0 {
                    return None;
                }
                if t < t1 {
                    t1 = t;
                }
            }
        }
    }
    Some(HatchSegment {
        x0: ax + t0 * dx,
        y0: ay + t0 * dy,
        x1: ax + t1 * dx,
        y1: ay + t1 * dy,
    })
}

#[cfg(test)]
#[path = "hatch_tests.rs"]
mod tests;
