// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Rotation CTM for [`loki_layout::PositionedItem::RotatedGroup`] in PDF space.
//!
//! `loki-vello` rotates a group in layout (y-down) space with
//! `M = T(pivot) · R(θ) · T(-pivot_local)` (see `loki-vello/src/scene.rs`). The
//! PDF renderer instead bakes a per-leaf y-flip `F: (x, y) → (x, page_h − y)`
//! into every leaf, so a group's children are already emitted in PDF (y-up)
//! space. To rotate them with the *same* geometry as on screen we set a content
//! CTM `C` and render the children with **zero** offset (the group's position is
//! folded into `M`). Because each child emits `q = F(p_local)`, the device point
//! is `C·q`; choosing `C = F · M · F` gives `C·q = F(M·p_local)` — the on-screen
//! placement flipped into PDF space. See the module tests for the derivation's
//! degenerate (θ = 0) check.

/// A 2-D affine as PDF's `[a b c d e f]`: it maps `(x, y)` to
/// `(a·x + c·y + e, b·x + d·y + f)`.
type Mat = [f64; 6];

/// `g ∘ h` — the matrix whose application equals applying `h` then `g`.
fn compose(g: Mat, h: Mat) -> Mat {
    [
        g[0] * h[0] + g[2] * h[1],
        g[1] * h[0] + g[3] * h[1],
        g[0] * h[2] + g[2] * h[3],
        g[1] * h[2] + g[3] * h[3],
        g[0] * h[4] + g[2] * h[5] + g[4],
        g[1] * h[4] + g[3] * h[5] + g[5],
    ]
}

fn translate(tx: f64, ty: f64) -> Mat {
    [1.0, 0.0, 0.0, 1.0, tx, ty]
}

/// Rotation matching kurbo / `loki-vello`'s `Affine::rotate(θ)` in y-down space.
fn rotate(theta: f64) -> Mat {
    let (s, c) = theta.sin_cos();
    [c, s, -s, c, 0.0, 0.0]
}

/// The per-leaf y-flip `F: (x, y) → (x, page_h − y)`.
fn reflect_y(page_h: f64) -> Mat {
    [1.0, 0.0, 0.0, -1.0, 0.0, page_h]
}

/// Build the content CTM for a rotated group.
///
/// `(abs_x, abs_y)` is the group's absolute content position — the area offset
/// (margins for content, zero for header/footer) plus the group origin;
/// `content_width/height` are the group's unrotated extents. The returned matrix
/// is passed to `Content::transform`, after which the group's children render
/// with a **zero** offset.
pub(crate) fn rotated_group_ctm(
    abs_x: f32,
    abs_y: f32,
    degrees: f32,
    content_width: f32,
    content_height: f32,
    page_h: f32,
) -> [f32; 6] {
    let (ox, oy) = (abs_x as f64, abs_y as f64);
    let cx_local = content_width as f64 / 2.0;
    let cy_local = content_height as f64 / 2.0;

    // After a quarter turn the bounding box's width/height swap, so the pivot's
    // physical offset uses the swapped half-extents (mirrors loki-vello).
    let (px, py) = match degrees as i32 {
        90 | 270 => (ox + cy_local, oy + cx_local),
        _ => (ox + cx_local, oy + cy_local),
    };

    let theta = (degrees as f64).to_radians();
    // M = T(pivot_physical) · R(θ) · T(-pivot_local)
    let m = compose(
        translate(px, py),
        compose(rotate(theta), translate(-cx_local, -cy_local)),
    );
    // C = F · M · F
    let f = reflect_y(page_h as f64);
    let c = compose(f, compose(m, f));
    [
        c[0] as f32,
        c[1] as f32,
        c[2] as f32,
        c[3] as f32,
        c[4] as f32,
        c[5] as f32,
    ]
}

#[cfg(test)]
#[path = "page_rotate_tests.rs"]
mod tests;
