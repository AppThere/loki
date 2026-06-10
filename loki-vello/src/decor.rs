// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Text decoration rendering (underline, strikethrough, overline).
//!
//! Translates a [`loki_layout::PositionedDecoration`] into a single Vello
//! stroke call.

use loki_layout::PositionedDecoration;

/// Paint a text decoration line (underline, strikethrough, or overline).
///
/// Decorations with zero or negative `width` or `thickness` are silently
/// skipped.
pub fn paint_decoration(scene: &mut vello::Scene, item: &PositionedDecoration, scale: f32) {
    if item.width <= 0.0 || item.thickness <= 0.0 {
        return;
    }

    // Centre the stroke on the middle of the decoration stripe.
    //
    // item.y is the TOP edge of the decoration area in screen Y-down space
    // (computed in loki-layout/src/para.rs by negating the skrifa Y-up offset).
    // A Vello/Kurbo stroke is centred on the path, so drawing at
    // item.y + thickness/2 fills exactly [item.y, item.y + thickness].
    let y = item.y + item.thickness / 2.0;

    let x0 = (item.x * scale) as f64;
    let x1 = ((item.x + item.width) * scale) as f64;
    let y_scaled = (y * scale) as f64;

    let line = kurbo::Line::new((x0, y_scaled), (x1, y_scaled));
    let brush = crate::color::to_brush(&item.color);

    scene.stroke(
        &kurbo::Stroke::new((item.thickness * scale) as f64),
        kurbo::Affine::IDENTITY,
        &brush,
        None,
        &line,
    );
}
