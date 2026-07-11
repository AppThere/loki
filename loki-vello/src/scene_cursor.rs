// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Caret, selection-highlight, and selection-handle painting. Split from
//! `scene.rs` (file-size ceiling) when the caret gained rotation support
//! (deferred-features 4b.5 tail: the caret *line* now tilts with rotated
//! table-cell text instead of staying upright).

use vello::kurbo::Affine;
use vello::peniko::{Brush, Color, Fill};

use loki_layout::{CursorRect, PageParagraphData};

use crate::scene::{SelectionHandle, SelectionRect};

// Selection-handle dimensions (in layout points).
const HANDLE_STEM_HEIGHT: f32 = 24.0;
const HANDLE_CIRCLE_RADIUS: f32 = 8.0;

/// Affine mapping paragraph-local caret/selection coordinates to physical
/// scene pixels for `para_data`'s paragraph.
///
/// For plain paragraphs this is the familiar `scale · translate(content_origin
/// + origin)`. For rotated table-cell content it composes the cell's
/// [`CellRotation`](loki_layout::CellRotation) affine (`page = pivot_page +
/// Rot(deg)·(local − pivot_local)`, the same transform the content itself is
/// painted with), so the caret line and selection fills tilt with the text.
pub(crate) fn cursor_paint_transform(
    para_data: Option<&PageParagraphData>,
    content_origin: (f32, f32),
    scale: f32,
) -> Affine {
    let origin = para_data.map(|p| p.origin).unwrap_or((0.0, 0.0));
    match para_data.and_then(|p| p.rotation) {
        Some(rot) => {
            Affine::scale(scale as f64)
                * Affine::translate((content_origin.0 as f64, content_origin.1 as f64))
                * Affine::translate((rot.pivot_page.0 as f64, rot.pivot_page.1 as f64))
                * Affine::rotate((rot.degrees as f64).to_radians())
                * Affine::translate((
                    (origin.0 - rot.pivot_local.0) as f64,
                    (origin.1 - rot.pivot_local.1) as f64,
                ))
        }
        None => {
            Affine::scale(scale as f64)
                * Affine::translate((
                    (content_origin.0 + origin.0) as f64,
                    (content_origin.1 + origin.1) as f64,
                ))
        }
    }
}

/// Paint a cursor line, optional selection highlight rects, and optional mobile
/// selection handles into the scene.
///
/// All coordinates are in paragraph-local layout points. `offset` is the
/// paragraph's origin in scene coordinates (content-area origin + paragraph
/// origin from `PageEditingData`). `scale` converts layout points to physical
/// pixels. Rotation-aware callers build the affine with
/// [`cursor_paint_transform`] and call [`paint_cursor_transformed`] directly.
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
    let transform =
        Affine::scale(scale as f64) * Affine::translate((offset.0 as f64, offset.1 as f64));
    paint_cursor_transformed(
        scene,
        cursor_rect,
        selection_rects,
        selection_handles,
        transform,
    );
}

/// [`paint_cursor`] with an explicit paragraph-local → scene transform, so a
/// rotated table cell's caret and selection render tilted with its text.
pub(crate) fn paint_cursor_transformed(
    scene: &mut vello::Scene,
    cursor_rect: &CursorRect,
    selection_rects: &[SelectionRect],
    selection_handles: &[SelectionHandle],
    transform: Affine,
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
        if sel.width <= 0.0 || sel.height <= 0.0 {
            continue;
        }
        scene.fill(
            Fill::NonZero,
            transform,
            &sel_brush,
            None,
            &vello::kurbo::Rect::new(
                sel.x as f64,
                sel.y as f64,
                (sel.x + sel.width) as f64,
                (sel.y + sel.height) as f64,
            ),
        );
    }

    // ── Cursor line ───────────────────────────────────────────────────────────
    // 2-point-wide vertical bar in the document accent colour.
    if cursor_rect.height > 0.0 {
        scene.fill(
            Fill::NonZero,
            transform,
            &accent_brush,
            None,
            &vello::kurbo::Rect::new(
                cursor_rect.x as f64,
                cursor_rect.y as f64,
                (cursor_rect.x + 2.0) as f64,
                (cursor_rect.y + cursor_rect.height) as f64,
            ),
        );
    }

    // ── Mobile selection handles ──────────────────────────────────────────────
    // Each handle is a teardrop: a 2-pt-wide vertical stem descending from the
    // selection edge, with a filled circle at the bottom.  Rendered only when
    // the caller populates `selection_handles` (iOS/Android only).
    for handle in selection_handles {
        let stem_w = 2.0_f32;

        // Stem: 2-pt wide rectangle descending from (tip_x, tip_y).
        scene.fill(
            Fill::NonZero,
            transform,
            &accent_brush,
            None,
            &vello::kurbo::Rect::new(
                handle.tip_x as f64,
                handle.tip_y as f64,
                (handle.tip_x + stem_w) as f64,
                (handle.tip_y + HANDLE_STEM_HEIGHT) as f64,
            ),
        );

        // Circle: centred horizontally on the stem, at the bottom.
        let cx = (handle.tip_x + stem_w / 2.0) as f64;
        let cy = (handle.tip_y + HANDLE_STEM_HEIGHT + HANDLE_CIRCLE_RADIUS) as f64;
        scene.fill(
            Fill::NonZero,
            transform,
            &accent_brush,
            None,
            &vello::kurbo::Circle::new((cx, cy), HANDLE_CIRCLE_RADIUS as f64),
        );
    }
}

#[cfg(test)]
#[path = "scene_cursor_tests.rs"]
mod tests;
