// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Filled and bordered rectangle rendering.
//!
//! Translates [`loki_layout::PositionedRect`] and
//! [`loki_layout::PositionedBorderRect`] into Vello `fill` / `stroke` calls.

use loki_layout::{BorderEdge, LayoutRect, PositionedBorderRect, PositionedHatch, PositionedRect};

/// Paint a `w:shd` hatch: the optional background fill, then each clipped hatch
/// line stroked at the pattern's line width.
pub fn paint_hatch(scene: &mut vello::Scene, item: &PositionedHatch, scale: f32) {
    if let Some(fill) = item.fill {
        paint_filled_rect(
            scene,
            &PositionedRect {
                rect: item.rect,
                color: fill,
            },
            scale,
        );
    }
    let brush = crate::color::to_brush(&item.color);
    let stroke = kurbo::Stroke::new(f64::from(item.line_width() * scale));
    let s = f64::from(scale);
    for seg in item.segments() {
        let line = kurbo::Line::new(
            (f64::from(seg.x0) * s, f64::from(seg.y0) * s),
            (f64::from(seg.x1) * s, f64::from(seg.y1) * s),
        );
        scene.stroke(&stroke, kurbo::Affine::IDENTITY, &brush, None, &line);
    }
}

/// Paint a filled rectangle into the scene.
pub fn paint_filled_rect(scene: &mut vello::Scene, item: &PositionedRect, scale: f32) {
    let rect = scale_rect(&item.rect, scale);
    let brush = crate::color::to_brush(&item.color);
    scene.fill(
        peniko::Fill::NonZero,
        kurbo::Affine::IDENTITY,
        &brush,
        None,
        &rect,
    );
}

/// Paint a bordered rectangle (stroked edges, no fill) into the scene.
///
/// Each edge may have an independent color and width. Absent edges (where
/// the `Option` is `None`) are not drawn. Edges with `width <= 0.0` are also
/// skipped.
pub fn paint_border_rect(scene: &mut vello::Scene, item: &PositionedBorderRect, scale: f32) {
    let rect = scale_rect(&item.rect, scale);
    paint_border_edge(
        scene,
        item.top.as_ref(),
        rect.x0,
        rect.y0,
        rect.x1,
        rect.y0,
        scale,
    );
    paint_border_edge(
        scene,
        item.right.as_ref(),
        rect.x1,
        rect.y0,
        rect.x1,
        rect.y1,
        scale,
    );
    paint_border_edge(
        scene,
        item.bottom.as_ref(),
        rect.x0,
        rect.y1,
        rect.x1,
        rect.y1,
        scale,
    );
    paint_border_edge(
        scene,
        item.left.as_ref(),
        rect.x0,
        rect.y0,
        rect.x0,
        rect.y1,
        scale,
    );
}

fn paint_border_edge(
    scene: &mut vello::Scene,
    edge: Option<&BorderEdge>,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    scale: f32,
) {
    let Some(edge) = edge else { return };
    if edge.width <= 0.0 {
        return;
    }
    let path = kurbo::Line::new((x0, y0), (x1, y1));
    let brush = crate::color::to_brush(&edge.color);
    scene.stroke(
        &kurbo::Stroke::new((edge.width * scale) as f64),
        kurbo::Affine::IDENTITY,
        &brush,
        None,
        &path,
    );
}

/// Convert a [`LayoutRect`] to a [`kurbo::Rect`] with the scale factor applied.
fn scale_rect(r: &LayoutRect, scale: f32) -> kurbo::Rect {
    kurbo::Rect::new(
        (r.x() * scale) as f64,
        (r.y() * scale) as f64,
        (r.max_x() * scale) as f64,
        (r.max_y() * scale) as f64,
    )
}
