// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the caret/selection paint transform (rotated-cell caret,
//! deferred-features 4b.5 tail).

use std::sync::Arc;

use vello::kurbo::Point;

use loki_layout::{CellRotation, CursorRect, PageParagraphData, ParagraphLayout};

use super::{cursor_paint_transform, paint_cursor};
use crate::scene::{SelectionHandle, SelectionHandleKind, SelectionRect};

/// Minimal paragraph editing entry at `origin`, optionally rotated.
fn para(origin: (f32, f32), rotation: Option<CellRotation>) -> PageParagraphData {
    let layout = ParagraphLayout {
        height: 16.0,
        width: 35.0,
        items: vec![],
        first_baseline: 10.0,
        last_baseline: 10.0,
        line_boundaries: Vec::new(),
        parley_layout: None,
        orig_to_clean: Vec::new(),
        clean_to_orig: Vec::new(),
        indent_start: 0.0,
        indent_hanging: 0.0,
        drop_lines: 0,
        drop_shift: 0.0,
    };
    PageParagraphData {
        block_index: 0,
        path: Vec::new(),
        layout: Arc::new(layout),
        origin,
        rotation,
    }
}

#[test]
fn unrotated_transform_is_translate_then_scale() {
    let p = para((3.0, 4.0), None);
    let t = cursor_paint_transform(Some(&p), (10.0, 20.0), 2.0);
    let mapped = t * Point::new(1.0, 2.0);
    // (content_origin + origin + local) * scale.
    assert!((mapped.x - 28.0).abs() < 1e-6, "{mapped:?}");
    assert!((mapped.y - 52.0).abs() < 1e-6, "{mapped:?}");
}

#[test]
fn missing_paragraph_falls_back_to_content_origin() {
    let t = cursor_paint_transform(None, (10.0, 20.0), 1.0);
    let mapped = t * Point::new(5.0, 6.0);
    assert!((mapped.x - 15.0).abs() < 1e-6, "{mapped:?}");
    assert!((mapped.y - 26.0).abs() < 1e-6, "{mapped:?}");
}

#[test]
fn rotated_transform_matches_the_cell_rotation_affine() {
    // The paint transform must send a paragraph-local point to exactly where
    // CellRotation::local_to_page puts it (plus content origin, times scale) —
    // the same affine the rotated cell's content is painted with.
    let rot = CellRotation {
        degrees: 90.0,
        pivot_local: (20.0, 5.0),
        pivot_page: (60.0, 40.0),
    };
    let origin = (2.0, 3.0);
    let p = para(origin, Some(rot));
    let (content_x, content_y) = (12.0, 34.0);
    let scale = 1.5_f64;
    let t = cursor_paint_transform(Some(&p), (content_x, content_y), scale as f32);

    let (local_x, local_y) = (7.0_f32, 2.0_f32);
    let (page_x, page_y) = rot.local_to_page(origin.0 + local_x, origin.1 + local_y);
    let expected = (
        (content_x + page_x) as f64 * scale,
        (content_y + page_y) as f64 * scale,
    );
    let mapped = t * Point::new(local_x as f64, local_y as f64);
    assert!(
        (mapped.x - expected.0).abs() < 1e-3,
        "{mapped:?} vs {expected:?}"
    );
    assert!(
        (mapped.y - expected.1).abs() < 1e-3,
        "{mapped:?} vs {expected:?}"
    );
}

#[test]
fn paint_cursor_smoke_all_shapes() {
    // The public offset/scale wrapper still paints every shape kind without
    // panicking (behavioural equivalence guard for the transform refactor).
    let mut scene = vello::Scene::new();
    paint_cursor(
        &mut scene,
        &CursorRect {
            x: 1.0,
            y: 2.0,
            height: 14.0,
        },
        &[SelectionRect {
            x: 0.0,
            y: 0.0,
            width: 30.0,
            height: 16.0,
        }],
        &[SelectionHandle {
            tip_x: 5.0,
            tip_y: 16.0,
            kind: SelectionHandleKind::Focus,
        }],
        (10.0, 20.0),
        2.0,
    );
}
