// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Text decoration rendering (underline, strikethrough, overline).
//!
//! Translates a [`loki_layout::PositionedDecoration`] into a single Vello
//! stroke call.

use loki_layout::items::DecorationStyle;
use loki_layout::{DecorationKind, PositionedDecoration};

/// Paint a text decoration (underline, strikethrough, overline, or the wavy
/// spelling squiggle) honouring its [`DecorationStyle`] — solid, double,
/// dotted, dashed, wave, or thick.
///
/// Decorations with zero or negative `width` or `thickness` are silently
/// skipped.
pub fn paint_decoration(scene: &mut vello::Scene, item: &PositionedDecoration, scale: f32) {
    if item.width <= 0.0 || item.thickness <= 0.0 {
        return;
    }

    let brush = crate::color::to_brush(&item.color);
    let t = (item.thickness * scale).max(0.5) as f64;

    // Spelling squiggles and `Wave` underlines share the wavy path.
    if item.kind == DecorationKind::Spelling || item.style == DecorationStyle::Wave {
        scene.stroke(
            &kurbo::Stroke::new(t),
            kurbo::Affine::IDENTITY,
            &brush,
            None,
            &squiggle_path(item, scale),
        );
        return;
    }

    // Centre the stroke on the middle of the decoration stripe.
    //
    // item.y is the TOP edge of the decoration area in screen Y-down space
    // (computed in loki-layout/src/para.rs by negating the skrifa Y-up offset).
    // A Vello/Kurbo stroke is centred on the path, so drawing at
    // item.y + thickness/2 fills exactly [item.y, item.y + thickness].
    let x0 = (item.x * scale) as f64;
    let x1 = ((item.x + item.width) * scale) as f64;
    let y = ((item.y + item.thickness / 2.0) * scale) as f64;

    let line = |scene: &mut vello::Scene, stroke: &kurbo::Stroke, yy: f64| {
        scene.stroke(
            stroke,
            kurbo::Affine::IDENTITY,
            &brush,
            None,
            &kurbo::Line::new((x0, yy), (x1, yy)),
        );
    };

    match item.style {
        DecorationStyle::Solid => line(scene, &kurbo::Stroke::new(t), y),
        DecorationStyle::Thick => line(scene, &kurbo::Stroke::new(t * 2.0), y),
        DecorationStyle::Double => {
            // Two thin parallel lines; the second sits just below the band.
            let stroke = kurbo::Stroke::new(t);
            line(scene, &stroke, y);
            line(scene, &stroke, y + (t * 2.0).max(1.0));
        }
        DecorationStyle::Dotted => {
            let stroke = kurbo::Stroke::new(t)
                .with_caps(kurbo::Cap::Round)
                .with_dashes(0.0, [t, t * 2.0]);
            line(scene, &stroke, y);
        }
        DecorationStyle::Dashed => {
            let stroke = kurbo::Stroke::new(t).with_dashes(0.0, [t * 3.0, t * 2.0]);
            line(scene, &stroke, y);
        }
        // `Wave` is handled above; enum is non-exhaustive so keep a fallback.
        _ => line(scene, &kurbo::Stroke::new(t), y),
    }
}

/// Builds a wavy path for a spelling squiggle across the decoration's width.
///
/// The wave amplitude is tied to the line thickness so it scales with zoom; one
/// full period spans roughly four thicknesses, approximating the squiggle of a
/// desktop word processor.
fn squiggle_path(item: &PositionedDecoration, scale: f32) -> kurbo::BezPath {
    let amplitude = (item.thickness * scale) as f64;
    let period = amplitude * 4.0;
    let x0 = (item.x * scale) as f64;
    let x1 = ((item.x + item.width) * scale) as f64;
    // Centre the wave within the decoration band so it stays below the glyphs.
    let y_mid = ((item.y + item.thickness / 2.0) * scale) as f64 + amplitude;

    let mut path = kurbo::BezPath::new();
    path.move_to((x0, y_mid));
    // Step in half-periods, alternating the control point above/below the mid
    // line to trace a continuous zig-zag of quadratic curves.
    let half = (period / 2.0).max(1.0);
    let mut x = x0;
    let mut up = true;
    while x < x1 {
        let next = (x + half).min(x1);
        let ctrl_y = if up {
            y_mid - amplitude
        } else {
            y_mid + amplitude
        };
        path.quad_to(((x + next) / 2.0, ctrl_y), (next, y_mid));
        x = next;
        up = !up;
    }
    path
}
