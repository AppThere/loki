// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Per-item painting: each arm mirrors its `loki-vello` twin (same
//! origin/offset/scale math, same skip rules), targeting
//! `vello_cpu::RenderContext` instead of `vello::Scene`.

use loki_layout::{
    BorderEdge, DecorationKind, LayoutColor, PositionedBorderRect, PositionedDecoration,
    PositionedGlyphRun, PositionedImage, PositionedItem, PositionedRect,
};
use vello_cpu::kurbo::{Affine, BezPath, Line, Rect, Shape, Stroke};
use vello_cpu::{RenderContext, Resources, color::AlphaColor, peniko};

/// The grey placeholder `loki-vello` paints for unresolved images.
const IMAGE_PLACEHOLDER: LayoutColor = LayoutColor {
    r: 0.8,
    g: 0.8,
    b: 0.8,
    a: 1.0,
};

fn to_color(c: &LayoutColor) -> AlphaColor<vello_cpu::color::Srgb> {
    AlphaColor::new([c.r, c.g, c.b, c.a])
}

/// Paints a slice of items at `(offset, scale)` — the CPU twin of
/// `loki_vello::paint_items`.
pub(crate) fn paint_items(
    ctx: &mut RenderContext,
    resources: &mut Resources,
    items: &[PositionedItem],
    scale: f32,
    offset: (f32, f32),
) {
    for item in items {
        match item {
            PositionedItem::GlyphRun(run) => paint_glyph_run(ctx, resources, run, scale, offset),
            PositionedItem::FilledRect(r) | PositionedItem::HorizontalRule(r) => {
                paint_filled_rect(ctx, r, scale, offset);
            }
            PositionedItem::BorderRect(b) => paint_border_rect(ctx, b, scale, offset),
            PositionedItem::Decoration(d) => paint_decoration(ctx, d, scale, offset),
            PositionedItem::Image(img) => paint_image_placeholder(ctx, img, scale, offset),
            PositionedItem::ClippedGroup { clip_rect, items } => {
                let mut path = BezPath::new();
                path.extend(
                    Rect::new(
                        f64::from((clip_rect.x() + offset.0) * scale),
                        f64::from((clip_rect.y() + offset.1) * scale),
                        f64::from((clip_rect.max_x() + offset.0) * scale),
                        f64::from((clip_rect.max_y() + offset.1) * scale),
                    )
                    .path_elements(0.1),
                );
                ctx.push_clip_layer(&path);
                paint_items(ctx, resources, items, scale, offset);
                ctx.pop_layer();
            }
            PositionedItem::RotatedGroup {
                origin,
                degrees,
                content_width,
                content_height,
                items,
            } => {
                // Mirrors loki-vello's rotated-cell math: rotate the locally
                // painted children about the physical cell centre.
                let ox = origin.x + offset.0;
                let oy = origin.y + offset.1;
                let cx_local = content_width / 2.0;
                let cy_local = content_height / 2.0;
                let (cx_physical, cy_physical) = match *degrees as i32 {
                    90 | 270 => (ox + cy_local, oy + cx_local),
                    _ => (ox + cx_local, oy + cy_local),
                };
                let transform = Affine::translate((
                    f64::from(cx_physical * scale),
                    f64::from(cy_physical * scale),
                )) * Affine::rotate(f64::from(*degrees).to_radians())
                    * Affine::translate((
                        f64::from(-cx_local * scale),
                        f64::from(-cy_local * scale),
                    ));
                let saved = *ctx.transform();
                ctx.set_transform(transform * saved);
                paint_items(ctx, resources, items, scale, (0.0, 0.0));
                ctx.set_transform(saved);
            }
            // `PositionedItem` is #[non_exhaustive]; ignore unknown variants
            // exactly as the GPU painter does.
            _ => {}
        }
    }
}

/// CPU twin of `loki_vello::glyph::paint_glyph_run`: baseline-origin
/// translate, per-glyph offsets scaled, `.notdef` skipped, hinting off.
fn paint_glyph_run(
    ctx: &mut RenderContext,
    resources: &mut Resources,
    run: &PositionedGlyphRun,
    scale: f32,
    offset: (f32, f32),
) {
    if run.glyphs.is_empty() || run.font_data.is_empty() {
        return;
    }
    let blob = peniko::Blob::new(run.font_data.clone());
    let font = peniko::FontData::new(blob, run.font_index);

    let saved = *ctx.transform();
    ctx.set_transform(
        saved
            * Affine::translate((
                f64::from((run.origin.x + offset.0) * scale),
                f64::from((run.origin.y + offset.1) * scale),
            )),
    );
    ctx.set_paint(to_color(&run.color));

    let glyphs = run
        .glyphs
        .iter()
        .filter(|g| g.id != 0)
        .map(|g| vello_cpu::Glyph {
            id: u32::from(g.id),
            x: g.x * scale,
            y: g.y * scale,
        })
        .collect::<Vec<_>>();

    // Apply the run's variable-font instance (e.g. Arimo `wght=700` for bold
    // Arial). Empty for static faces, where glifo uses the default master.
    ctx.glyph_run(resources, &font)
        .font_size(run.font_size * scale)
        .normalized_coords(&run.normalized_coords)
        .hint(false)
        .fill_glyphs(glyphs.into_iter());

    ctx.set_transform(saved);
}

/// CPU twin of `loki_vello::rect::paint_filled_rect`.
pub(crate) fn paint_filled_rect(
    ctx: &mut RenderContext,
    item: &PositionedRect,
    scale: f32,
    offset: (f32, f32),
) {
    ctx.set_paint(to_color(&item.color));
    ctx.fill_rect(&Rect::new(
        f64::from((item.rect.x() + offset.0) * scale),
        f64::from((item.rect.y() + offset.1) * scale),
        f64::from((item.rect.max_x() + offset.0) * scale),
        f64::from((item.rect.max_y() + offset.1) * scale),
    ));
}

/// CPU twin of `loki_vello::rect::paint_border_rect`: each present edge is a
/// centred stroke along its side; `width <= 0` edges skipped.
fn paint_border_rect(
    ctx: &mut RenderContext,
    item: &PositionedBorderRect,
    scale: f32,
    offset: (f32, f32),
) {
    let x0 = f64::from((item.rect.x() + offset.0) * scale);
    let y0 = f64::from((item.rect.y() + offset.1) * scale);
    let x1 = f64::from((item.rect.max_x() + offset.0) * scale);
    let y1 = f64::from((item.rect.max_y() + offset.1) * scale);
    paint_edge(ctx, item.top.as_ref(), (x0, y0), (x1, y0), scale);
    paint_edge(ctx, item.right.as_ref(), (x1, y0), (x1, y1), scale);
    paint_edge(ctx, item.bottom.as_ref(), (x0, y1), (x1, y1), scale);
    paint_edge(ctx, item.left.as_ref(), (x0, y0), (x0, y1), scale);
}

fn paint_edge(
    ctx: &mut RenderContext,
    edge: Option<&BorderEdge>,
    from: (f64, f64),
    to: (f64, f64),
    scale: f32,
) {
    let Some(edge) = edge else { return };
    if edge.width <= 0.0 {
        return;
    }
    ctx.set_paint(to_color(&edge.color));
    ctx.set_stroke(Stroke::new(f64::from(edge.width * scale)));
    let mut path = BezPath::new();
    path.extend(Line::new(from, to).path_elements(0.1));
    ctx.stroke_path(&path);
}

/// CPU twin of `loki_vello::decor::paint_decoration`: the stroke is centred
/// on the middle of the decoration stripe; the spelling squiggle uses the
/// same wave geometry.
fn paint_decoration(
    ctx: &mut RenderContext,
    item: &PositionedDecoration,
    scale: f32,
    offset: (f32, f32),
) {
    if item.width <= 0.0 || item.thickness <= 0.0 {
        return;
    }
    ctx.set_paint(to_color(&item.color));
    ctx.set_stroke(Stroke::new(f64::from(item.thickness * scale)));

    let x = item.x + offset.0;
    let y = item.y + offset.1;
    let mut path = BezPath::new();
    if item.kind == DecorationKind::Spelling {
        // Squiggle: half-wave quads with a 2pt period, matching loki-vello.
        let amplitude = f64::from(item.thickness * scale);
        let period = f64::from(2.0 * scale);
        let x0 = f64::from(x * scale);
        let x1 = f64::from((x + item.width) * scale);
        let yc = f64::from((y + item.thickness) * scale);
        path.move_to((x0, yc));
        let mut cx = x0;
        let mut up = true;
        while cx < x1 {
            let next = (cx + period).min(x1);
            let ctrl_y = if up { yc - amplitude } else { yc + amplitude };
            path.quad_to(((cx + next) / 2.0, ctrl_y), (next, yc));
            up = !up;
            cx = next;
        }
    } else {
        let yc = f64::from((y + item.thickness / 2.0) * scale);
        path.extend(
            Line::new(
                (f64::from(x * scale), yc),
                (f64::from((x + item.width) * scale), yc),
            )
            .path_elements(0.1),
        );
    }
    ctx.stroke_path(&path);
}

/// TODO(conformance-render): decode and draw the actual image; today this is
/// the same grey placeholder `loki-vello` paints for unresolved images, so
/// image-bearing fixtures diff on the placeholder box, not garbage.
fn paint_image_placeholder(
    ctx: &mut RenderContext,
    img: &PositionedImage,
    scale: f32,
    offset: (f32, f32),
) {
    paint_filled_rect(
        ctx,
        &PositionedRect {
            rect: img.rect,
            color: IMAGE_PLACEHOLDER,
        },
        scale,
        offset,
    );
}
