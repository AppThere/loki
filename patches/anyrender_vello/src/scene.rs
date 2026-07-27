use anyrender::{CustomPaint, NormalizedCoord, Paint, PaintRef, PaintScene};
use kurbo::{Affine, Rect, Shape, Stroke};
use peniko::{BlendMode, BrushRef, Color, Fill, FontData, ImageBrush, StyleRef};
use rustc_hash::FxHashMap;
use vello::Renderer as VelloRenderer;

use crate::{CustomPaintSource, custom_paint_source::CustomPaintCtx};

pub struct VelloScenePainter<'r, 's> {
    pub(crate) renderer: Option<&'r mut VelloRenderer>,
    pub(crate) custom_paint_sources: Option<&'r mut FxHashMap<u64, Box<dyn CustomPaintSource>>>,
    pub(crate) inner: &'s mut vello::Scene,
}

impl VelloScenePainter<'_, '_> {
    pub fn new<'s>(scene: &'s mut vello::Scene) -> VelloScenePainter<'static, 's> {
        VelloScenePainter {
            renderer: None,
            custom_paint_sources: None,
            inner: scene,
        }
    }

    fn render_custom_source(&mut self, custom_paint: CustomPaint) -> Option<peniko::ImageBrush> {
        let (Some(renderer), Some(custom_paint_sources)) =
            (&mut self.renderer, &mut self.custom_paint_sources)
        else {
            return None;
        };

        let CustomPaint {
            source_id,
            width,
            height,
            scale,
        } = custom_paint;

        // Render custom paint source
        let source = custom_paint_sources.get_mut(&source_id)?;
        let ctx = CustomPaintCtx::new(renderer);
        let texture_handle = source.render(ctx, width, height, scale)?;

        // Return dummy image
        Some(ImageBrush::new(texture_handle.0))
    }
}

impl PaintScene for VelloScenePainter<'_, '_> {
    fn reset(&mut self) {
        self.inner.reset();
    }

    fn push_layer(
        &mut self,
        blend: impl Into<BlendMode>,
        alpha: f32,
        transform: Affine,
        clip: &impl Shape,
    ) {
        self.inner.push_layer(blend, alpha, transform, clip);
    }

    fn push_clip_layer(&mut self, transform: Affine, clip: &impl Shape) {
        self.inner.push_clip_layer(transform, clip);
    }

    fn pop_layer(&mut self) {
        self.inner.pop_layer();
    }

    fn stroke<'a>(
        &mut self,
        style: &Stroke,
        transform: Affine,
        paint_ref: impl Into<PaintRef<'a>>,
        brush_transform: Option<Affine>,
        shape: &impl Shape,
    ) {
        let paint_ref: PaintRef<'_> = paint_ref.into();
        let brush_ref: BrushRef<'_> = paint_ref.into();
        self.inner
            .stroke(style, transform, brush_ref, brush_transform, shape);
    }

    fn fill<'a>(
        &mut self,
        style: Fill,
        transform: Affine,
        paint: impl Into<PaintRef<'a>>,
        brush_transform: Option<Affine>,
        shape: &impl Shape,
    ) {
        let paint: PaintRef<'_> = paint.into();

        let dummy_image: peniko::ImageBrush;
        // PATCH(loki): a custom paint source may return a texture *smaller* than
        // the box it fills. Upstream passes the image brush through with the
        // caller's `brush_transform` (which blitz-paint leaves `None` for a
        // canvas), so the image is sampled 1:1 and a smaller texture lands in
        // the top-left corner with the rest of the box showing the brush's
        // extend mode — not a scaled-down page, just a broken one.
        //
        // Loki's texture budget (Spec 08 T2.2) reduces the rasterisation scale
        // of off-centre pages under memory pressure, which is exactly that case:
        // the tile keeps its on-screen box and its texture shrinks. So when the
        // returned texture's dimensions differ from the requested ones, scale
        // the brush to compensate.
        //
        // Inert whenever a source returns a texture of the size it was asked
        // for, which is every source upstream has. See docs/patches.md.
        let mut brush_transform = brush_transform;
        let brush_ref: BrushRef<'_> = match paint {
            Paint::Solid(color) => BrushRef::Solid(color),
            Paint::Gradient(gradient) => BrushRef::Gradient(gradient),
            Paint::Image(image) => BrushRef::Image(image),
            Paint::Custom(custom_paint) => {
                let Some(custom_paint) = custom_paint.downcast_ref::<CustomPaint>() else {
                    return;
                };
                let requested = (custom_paint.width, custom_paint.height);
                let Some(image) = self.render_custom_source(*custom_paint) else {
                    return;
                };
                dummy_image = image;
                let got = (dummy_image.image.width, dummy_image.image.height);
                brush_transform = fit_brush_to_box(requested, got, brush_transform);
                BrushRef::Image(dummy_image.as_ref())
            }
        };

        self.inner
            .fill(style, transform, brush_ref, brush_transform, shape);
    }

    fn draw_glyphs<'a, 's: 'a>(
        &'a mut self,
        font: &'a FontData,
        font_size: f32,
        hint: bool,
        normalized_coords: &'a [NormalizedCoord],
        style: impl Into<StyleRef<'a>>,
        paint: impl Into<PaintRef<'a>>,
        brush_alpha: f32,
        transform: Affine,
        glyph_transform: Option<Affine>,
        glyphs: impl Iterator<Item = anyrender::Glyph>,
    ) {
        self.inner
            .draw_glyphs(font)
            .font_size(font_size)
            .hint(hint)
            .normalized_coords(normalized_coords)
            .brush(paint.into())
            .brush_alpha(brush_alpha)
            .transform(transform)
            .glyph_transform(glyph_transform)
            .draw(
                style,
                glyphs.map(|g: anyrender::Glyph| vello::Glyph {
                    id: g.id,
                    x: g.x,
                    y: g.y,
                }),
            );
    }

    fn draw_box_shadow(
        &mut self,
        transform: Affine,
        rect: Rect,
        brush: Color,
        radius: f64,
        std_dev: f64,
    ) {
        self.inner
            .draw_blurred_rounded_rect(transform, rect, brush, radius, std_dev);
    }
}

/// Brush transform that makes a texture of size `got` cover a box of size
/// `requested`, composed with whatever `existing` transform the caller supplied.
///
/// PATCH(loki): extracted from [`VelloScenePainter::fill`] so the correction can
/// be asserted without rendering. Spec 08 R28 was argued from a screen session —
/// "a mis-scaled tile would have been noticed and wasn't" — and that argument
/// does not hold: the same log shows a reduced tile replaced within ~2 ms, so a
/// *misplaced* tile had the same sub-frame window in which to go unseen. Absence
/// of a report across one frame is not evidence, and the R5a conclusion drawn
/// from that same 2 ms cannot be run in the opposite direction here.
///
/// So the gross case is settled deterministically instead. Sub-pixel correctness
/// — filtering, half-texel offsets at the edges — is not covered and remains
/// open, which is where it already was.
///
/// Returns `existing` unchanged when no correction applies, so a source that
/// returns the size it was asked for (every source upstream has) is untouched.
///
/// # Total, deliberately — no `debug_assert` on the shrink direction
///
/// Loki's budget only ever *reduces* rasterisation scale, so `got > requested`
/// should not arise, and asserting that is a tempting way to record the
/// assumption. It is the wrong instrument here. The assert would fire in the very
/// test that pins the oversized behaviour, so keeping both means keeping neither;
/// and a debug-only invariant on a rendering path means the release build — the
/// only one a reader ever runs — falls through to whatever the arithmetic does,
/// unexamined. A ratio is direction-agnostic at no extra cost, so the function
/// handles both and `an_oversized_texture_is_scaled_down_to_the_box` says which
/// direction is the exercised one.
fn fit_brush_to_box(
    requested: (u32, u32),
    got: (u32, u32),
    existing: Option<Affine>,
) -> Option<Affine> {
    // Degenerate sizes bail out rather than producing a transform, and both
    // directions matter for a different reason:
    //
    // - `got` zero divides by zero, giving an infinite or NaN transform. A NaN
    //   affine does not raise anything — it renders as nothing, or as garbage,
    //   which is the quiet-failure shape rather than a crash.
    // - `requested` zero yields scale 0, which is finite and therefore worse: it
    //   collapses the texture to a point and looks like a deliberate transform.
    //   A zero-area box has nothing to fill either way, so declining is honest.
    //
    // Reachable in principle whenever a page fails to rasterise or a tile is laid
    // out at zero size, neither of which the budget produces but neither of which
    // this function is in a position to rule out.
    if got == requested || got.0 == 0 || got.1 == 0 || requested.0 == 0 || requested.1 == 0 {
        return existing;
    }
    let sx = f64::from(requested.0) / f64::from(got.0);
    let sy = f64::from(requested.1) / f64::from(got.1);
    // `pre_scale` rather than `post_scale`: the texture-fitting scale belongs in
    // brush space, *inside* whatever the caller was already doing to the brush,
    // so a caller-supplied rotation or offset still applies to the fitted result.
    Some(
        existing
            .unwrap_or(Affine::IDENTITY)
            .pre_scale_non_uniform(sx, sy),
    )
}

#[cfg(test)]
#[path = "scene_brush_fit_tests.rs"]
mod brush_fit_tests;
