//! PATCH(loki): the deterministic half of Spec 08 R28.
//!
//! # The property, and why it is stated in texture coordinates
//!
//! Vello maps a point `p` in the brush's own space to the scene by
//! `transform * brush_transform * p`. For an image brush, brush space *is*
//! texture space: the texture occupies `(0,0)..(got.0, got.1)` there. So "the
//! texture covers the requested box" is exactly "the brush transform carries the
//! texture's far corner onto the box's far corner, and its origin onto the box's
//! origin" — two point mappings, no pixels involved.
//!
//! Asserting the corners rather than the scale factors matters: a correct pair of
//! factors applied on the wrong side of an existing transform, or with `post_`
//! instead of `pre_`, still reads as `sx = 0.5` while placing the texture
//! somewhere else entirely. The corner is the thing the reader would see.

use super::fit_brush_to_box;
use kurbo::{Affine, Point};

/// Where the brush transform sends a point of texture space.
fn maps(t: Option<Affine>, x: f64, y: f64) -> Point {
    t.unwrap_or(Affine::IDENTITY) * Point::new(x, y)
}

fn assert_covers(requested: (u32, u32), got: (u32, u32), t: Option<Affine>) {
    let origin = maps(t, 0.0, 0.0);
    assert!(
        origin.x.abs() < 1e-9 && origin.y.abs() < 1e-9,
        "texture origin landed at {origin:?}, not the box origin — the texture is \
         offset within its box",
    );
    let far = maps(t, f64::from(got.0), f64::from(got.1));
    assert!(
        (far.x - f64::from(requested.0)).abs() < 1e-9
            && (far.y - f64::from(requested.1)).abs() < 1e-9,
        "texture far corner landed at {far:?}, not ({}, {}) — the texture does not \
         span its box",
        requested.0,
        requested.1,
    );
}

/// The case the budget actually produces: a page rasterised at 0.75 and drawn
/// into its full-size box. This is the `raster_permille=750` line in the Phase 2
/// screen log, checked without a screen.
#[test]
fn a_reduced_scale_tile_spans_its_full_box() {
    let requested = (3173, 4490);
    let got = (2380, 3368);
    assert_covers(requested, got, fit_brush_to_box(requested, got, None));
}

/// Every rung of `RASTER_SCALE_LADDER`, since a tile may sit on any of them and
/// the non-integer ones round differently on each axis.
#[test]
fn every_ladder_rung_spans_its_box() {
    let requested = (1000u32, 1400u32);
    for scale in [0.75f64, 0.5, 0.35, 0.25] {
        let got = (
            (f64::from(requested.0) * scale).round() as u32,
            (f64::from(requested.1) * scale).round() as u32,
        );
        assert_covers(requested, got, fit_brush_to_box(requested, got, None));
    }
}

/// Non-uniform reduction — the axes must be corrected independently. A single
/// averaged factor would pass the uniform cases above and stretch this one.
#[test]
fn axes_are_corrected_independently() {
    let requested = (800, 600);
    let got = (400, 500);
    assert_covers(requested, got, fit_brush_to_box(requested, got, None));
}

/// The patch must be inert for every source that returns what it was asked for,
/// which is every source upstream has. `None` in, `None` out — not
/// `Some(IDENTITY)`, because that would make the patch's footprint visible in a
/// scene it has no business touching.
#[test]
fn an_exact_size_texture_leaves_the_brush_untouched() {
    assert_eq!(fit_brush_to_box((512, 512), (512, 512), None), None);
    let caller = Affine::translate((3.0, 4.0));
    assert_eq!(
        fit_brush_to_box((512, 512), (512, 512), Some(caller)),
        Some(caller)
    );
}

/// A caller-supplied brush transform must still apply *to the fitted result*.
/// `pre_scale` and `post_scale` both produce the right scale factors here and
/// disagree about where the texture lands, which is why the assertion is on the
/// corner: under `post_scale` the translation would be scaled too and the far
/// corner would miss by a factor of two.
#[test]
fn a_caller_transform_composes_outside_the_fit() {
    let requested = (400, 400);
    let got = (200, 200);
    let caller = Affine::translate((10.0, 20.0));
    let t = fit_brush_to_box(requested, got, Some(caller));
    assert_eq!(maps(t, 0.0, 0.0), Point::new(10.0, 20.0));
    assert_eq!(maps(t, 200.0, 200.0), Point::new(410.0, 420.0));
}

/// A zero-sized texture must not divide by zero. Nothing sensible can be drawn
/// either way; the requirement is that the brush is left alone rather than
/// carrying an infinity into the scene.
#[test]
fn a_zero_sized_texture_is_left_alone() {
    assert_eq!(fit_brush_to_box((400, 400), (0, 300), None), None);
    assert_eq!(fit_brush_to_box((400, 400), (300, 0), None), None);
}

/// A texture *larger* than the box. The budget never produces this today — scales
/// are at most 1.0 — but the correction is a ratio, not a reduction, and a future
/// supersampling path would depend on it holding in this direction too.
#[test]
fn an_oversized_texture_is_scaled_down_to_the_box() {
    let requested = (300, 300);
    let got = (600, 600);
    assert_covers(requested, got, fit_brush_to_box(requested, got, None));
}
