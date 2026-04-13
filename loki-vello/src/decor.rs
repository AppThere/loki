// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Text decoration rendering (underline, strikethrough, overline).
//!
//! Translates a [`loki_layout::PositionedDecoration`] into a single Vello
//! stroke call.

use loki_layout::{DecorationKind, PositionedDecoration};

/// Paint a text decoration line (underline, strikethrough, or overline).
///
/// Decorations with zero or negative `width` or `thickness` are silently
/// skipped.
pub fn paint_decoration(scene: &mut vello::Scene, item: &PositionedDecoration, scale: f32) {
    if item.width <= 0.0 || item.thickness <= 0.0 {
        return;
    }

    // Compute the y position of the decoration line relative to the baseline.
    let y = match item.kind {
        // Underline: draw below the baseline.
        DecorationKind::Underline => item.y + item.thickness,
        // Strikethrough: approximate the midline of the text.
        DecorationKind::Strikethrough => item.y - (item.thickness * 2.0),
        // Overline: draw above the text.
        DecorationKind::Overline => item.y - item.thickness,
        // Any future variant: fall back to baseline.
        _ => item.y,
    };

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
