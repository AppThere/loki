// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! A `w:shd` line/cross texture (`diagStripe`, `pct25`, …) reaches the
//! renderers as [`PositionedItem::HatchRect`]. This renderer's paint dispatch
//! had **no arm** for that variant, so every textured cell fell through the
//! catch-all and painted nothing — `acid2-docx.docx`'s "diagonal stripe" cell
//! rendered blank where Word fills it.
//!
//! The check that catches a missing arm is *ink where the hatch is*; the check
//! that catches a hatch degenerating into a plain fill is *paper still visible
//! between the lines*. Both are asserted, plus their inversion (no ink outside
//! the hatch rect).

use std::sync::Arc;

use loki_layout::{
    HatchPattern, LayoutColor, LayoutInsets, LayoutPage, LayoutRect, LayoutSize, PaginatedLayout,
    PositionedHatch, PositionedItem,
};
use loki_render_cpu::render_page;

/// Conformance DPI (matches `appthere_conformance::CONFORMANCE_DPI`).
const DPI: u32 = 144;
const SCALE: f32 = DPI as f32 / 72.0;

/// Page geometry: a 200×200pt page with 20pt margins, so the content area is
/// 160×160pt and content-local `(0, 0)` is device `(40, 40)`.
const MARGIN: f32 = 20.0;
/// The hatch occupies content-local `(10, 10)`–`(90, 90)`.
const HATCH_ORIGIN: f32 = 10.0;
const HATCH_SIDE: f32 = 80.0;

/// Pure red, so a hatch pixel is unambiguous against white paper.
const HATCH_COLOR: LayoutColor = LayoutColor {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

fn page_with(items: Vec<PositionedItem>) -> PaginatedLayout {
    let page_size = LayoutSize {
        width: 200.0,
        height: 200.0,
    };
    PaginatedLayout {
        page_size,
        pages: vec![Arc::new(LayoutPage {
            page_number: 1,
            page_size,
            margins: LayoutInsets {
                top: MARGIN,
                right: MARGIN,
                bottom: MARGIN,
                left: MARGIN,
            },
            content_items: items,
            header_items: vec![],
            footer_items: vec![],
            comment_items: vec![],
            header_height: 0.0,
            footer_height: 0.0,
            editing_data: None,
        })],
    }
}

fn hatch(fill: Option<LayoutColor>) -> PositionedItem {
    PositionedItem::HatchRect(PositionedHatch {
        rect: LayoutRect::new(HATCH_ORIGIN, HATCH_ORIGIN, HATCH_SIDE, HATCH_SIDE),
        fill,
        color: HATCH_COLOR,
        pattern: HatchPattern::DiagDown,
        thin: false,
    })
}

/// Device-pixel bounds of the hatch rect, margins included.
fn hatch_bounds() -> (u32, u32, u32, u32) {
    let x0 = ((MARGIN + HATCH_ORIGIN) * SCALE).round() as u32;
    let x1 = ((MARGIN + HATCH_ORIGIN + HATCH_SIDE) * SCALE).round() as u32;
    (x0, x0, x1, x1)
}

/// Ink inside the rect, unpainted paper inside it, and the furthest distance
/// (in device pixels) that any hatch ink strays beyond the rect.
fn ink_counts(img: &image::RgbaImage) -> (usize, usize, u32) {
    let (x0, y0, x1, y1) = hatch_bounds();
    let (mut inside_ink, mut inside_paper, mut overshoot) = (0, 0, 0);
    for (x, y, px) in img.enumerate_pixels() {
        let reddish = px.0[0] > 128 && px.0[1] < 128 && px.0[2] < 128;
        let white = px.0[0] > 240 && px.0[1] > 240 && px.0[2] > 240;
        // Inset by a pixel so stroke antialiasing on the boundary is not
        // scored as either side's evidence.
        if x > x0 && x < x1 - 1 && y > y0 && y < y1 - 1 {
            if reddish {
                inside_ink += 1;
            } else if white {
                inside_paper += 1;
            }
        } else if reddish {
            let d = [
                x0.saturating_sub(x),
                x.saturating_sub(x1),
                y0.saturating_sub(y),
                y.saturating_sub(y1),
            ]
            .into_iter()
            .max()
            .unwrap_or(0);
            overshoot = overshoot.max(d);
        }
    }
    (inside_ink, inside_paper, overshoot)
}

#[test]
fn hatch_rect_paints_lines_and_leaves_paper_between_them() {
    let layout = page_with(vec![hatch(None)]);
    let img = render_page(&layout, 0, DPI).expect("render");

    let (ink, paper, overshoot) = ink_counts(&img);

    // Without a `HatchRect` dispatch arm this is 0 — the defect this test
    // exists for.
    assert!(ink > 500, "hatch painted no lines (ink={ink})");
    // …and the inversion: a hatch is lines, not a flood fill, so unpainted
    // paper must remain between them.
    assert!(
        paper > ink,
        "hatch filled the rect solid (ink={ink} paper={paper})"
    );
    // Lines are clipped to the rect but *stroked* about that clip, so half a
    // stroke width may sit outside it. Anything beyond that is a real leak.
    assert!(
        overshoot <= 2,
        "hatch ink strays {overshoot}px past its rect"
    );
}

#[test]
fn hatch_rect_paints_its_background_fill_under_the_lines() {
    // Same hatch, now with an opaque white-ish blue background. The paper
    // between the lines must become the fill colour, proving `fill` is drawn
    // rather than ignored.
    let fill = LayoutColor {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
    let layout = page_with(vec![hatch(Some(fill))]);
    let img = render_page(&layout, 0, DPI).expect("render");

    let (x0, y0, x1, y1) = hatch_bounds();
    let mut blue = 0;
    let mut white = 0;
    for (x, y, px) in img.enumerate_pixels() {
        if x > x0 && x < x1 - 1 && y > y0 && y < y1 - 1 {
            if px.0[2] > 128 && px.0[0] < 128 {
                blue += 1;
            } else if px.0[0] > 240 && px.0[1] > 240 && px.0[2] > 240 {
                white += 1;
            }
        }
    }
    assert!(blue > 1000, "background fill not painted (blue={blue})");
    assert_eq!(white, 0, "paper still showing through an opaque fill");
}
