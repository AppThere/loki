// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Cursor line, selection highlight, and mobile selection-handle painting.

use vello::kurbo::Affine;
use vello::peniko::{Brush, Color, Fill};

use loki_layout::CursorRect;

use super::types::{HANDLE_CIRCLE_RADIUS, HANDLE_STEM_HEIGHT, SelectionHandle, SelectionRect};

/// Paint a cursor line, optional selection highlight rects, and optional mobile
/// selection handles into the scene.
///
/// All coordinates are in paragraph-local layout points. `offset` is the
/// paragraph's origin in scene coordinates (content-area origin + paragraph
/// origin from `PageEditingData`). `scale` converts layout points to physical
/// pixels.
///
/// The cursor is a 2-point-wide vertical line in the document accent colour.
/// Each selection rect is a semi-transparent blue fill.
/// Selection handles (teardrop: stem + circle) are drawn on mobile only —
/// the caller controls this via `#[cfg(target_os)]` before populating
/// `selection_handles`.
pub fn paint_cursor(
    scene: &mut vello::Scene,
    cursor_rect: &CursorRect,
    selection_rects: &[SelectionRect],
    selection_handles: &[SelectionHandle],
    offset: (f32, f32),
    scale: f32,
) {
    let accent_brush = Brush::Solid(Color::new([
        30.0 / 255.0,
        100.0 / 255.0,
        200.0 / 255.0,
        1.0,
    ]));

    // ── Selection highlight rects ─────────────────────────────────────────────
    // Painted before the cursor so the cursor line appears on top.
    let sel_brush = Brush::Solid(Color::new([
        30.0 / 255.0,
        100.0 / 255.0,
        200.0 / 255.0,
        60.0 / 255.0,
    ]));
    for sel in selection_rects {
        let x = (offset.0 + sel.x) * scale;
        let y = (offset.1 + sel.y) * scale;
        let w = sel.width * scale;
        let h = sel.height * scale;
        if w <= 0.0 || h <= 0.0 {
            continue;
        }
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &sel_brush,
            None,
            &vello::kurbo::Rect::new(x as f64, y as f64, (x + w) as f64, (y + h) as f64),
        );
    }

    // ── Cursor line ───────────────────────────────────────────────────────────
    // 2-point-wide vertical bar in the document accent colour.
    if cursor_rect.height > 0.0 {
        let x = (offset.0 + cursor_rect.x) * scale;
        let y = (offset.1 + cursor_rect.y) * scale;
        let h = cursor_rect.height * scale;
        let w = 2.0 * scale;
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &accent_brush,
            None,
            &vello::kurbo::Rect::new(x as f64, y as f64, (x + w) as f64, (y + h) as f64),
        );
    }

    // ── Mobile selection handles ──────────────────────────────────────────────
    // Each handle is a teardrop: a 2-pt-wide vertical stem descending from the
    // selection edge, with a filled circle at the bottom.  Rendered only when
    // the caller populates `selection_handles` (iOS/Android only).
    for handle in selection_handles {
        let tip_x = (offset.0 + handle.tip_x) * scale;
        let tip_y = (offset.1 + handle.tip_y) * scale;
        let stem_h = HANDLE_STEM_HEIGHT * scale;
        let stem_w = 2.0 * scale;
        let r = (HANDLE_CIRCLE_RADIUS * scale) as f64;

        // Stem: 2-pt wide rectangle descending from (tip_x, tip_y).
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &accent_brush,
            None,
            &vello::kurbo::Rect::new(
                tip_x as f64,
                tip_y as f64,
                (tip_x + stem_w) as f64,
                (tip_y + stem_h) as f64,
            ),
        );

        // Circle: centred horizontally on the stem, at the bottom.
        let cx = (tip_x + stem_w / 2.0) as f64;
        let cy = (tip_y + stem_h) as f64 + r;
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &accent_brush,
            None,
            &vello::kurbo::Circle::new((cx, cy), r),
        );
    }
}
