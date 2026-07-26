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
                if got != requested && got.0 > 0 && got.1 > 0 {
                    let sx = f64::from(requested.0) / f64::from(got.0);
                    let sy = f64::from(requested.1) / f64::from(got.1);
                    brush_transform = Some(
                        brush_transform
                            .unwrap_or(Affine::IDENTITY)
                            .pre_scale_non_uniform(sx, sy),
                    );
                }
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
