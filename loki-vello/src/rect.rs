// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Filled and bordered rectangle rendering.
//!
//! Translates [`loki_layout::PositionedRect`] and
//! [`loki_layout::PositionedBorderRect`] into Vello `fill` / `stroke` calls.

use loki_layout::{BorderEdge, LayoutRect, PositionedBorderRect, PositionedRect};

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
    paint_border_edge(scene, item.top.as_ref(), rect.x0, rect.y0, rect.x1, rect.y0, scale);
    paint_border_edge(scene, item.right.as_ref(), rect.x1, rect.y0, rect.x1, rect.y1, scale);
    paint_border_edge(scene, item.bottom.as_ref(), rect.x0, rect.y1, rect.x1, rect.y1, scale);
    paint_border_edge(scene, item.left.as_ref(), rect.x0, rect.y0, rect.x0, rect.y1, scale);
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
