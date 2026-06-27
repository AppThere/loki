// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! PATCH(loki): custom hover-tooltip overlay painted on top of the Blitz scene.
//!
//! Blitz / Stylo do not support `position: absolute` / `fixed`, so a hover
//! tooltip cannot be a DOM element. Instead the shell hit-tests the hovered
//! element for a `title` attribute and paints the tooltip directly into the
//! Vello scene *after* the DOM (`paint_scene`), anchored to the cursor. A
//! self-contained parley [`FontContext`] shapes the short label, so no DOM
//! node or style is involved — the overlay lives entirely outside Blitz.

use std::time::{Duration, Instant};

use anyrender::{Glyph, Paint, PaintScene};
use kurbo::{Affine, Rect, RoundedRect};
use parley::{
    FontContext, FontFamily, FontStack, GenericFamily, LayoutContext, PositionedLayoutItem,
    StyleProperty,
};
use peniko::{Color, Fill};

/// How long the cursor must rest on a titled element before its tooltip shows.
pub const HOVER_DELAY: Duration = Duration::from_millis(500);

const FONT_SIZE: f32 = 13.0;
const PAD_X: f64 = 8.0;
const PAD_Y: f64 = 5.0;
const CURSOR_DX: f64 = 14.0;
const CURSOR_DY: f64 = 18.0;
const RADIUS: f64 = 6.0;
const MAX_WIDTH: f32 = 360.0;
const EDGE: f64 = 4.0;

const BG: Color = Color::from_rgba8(38, 38, 44, 245);
const FG: Color = Color::from_rgba8(236, 236, 238, 255);
const SHADOW: Color = Color::from_rgba8(0, 0, 0, 90);

/// A pending or visible tooltip.
pub struct Tooltip {
    /// DOM node the `title` came from — used to detect when the hover leaves it.
    pub node_id: usize,
    /// The label text (the element's `title`).
    pub text: String,
    /// Cursor anchor in logical (CSS) pixels.
    pub anchor: (f32, f32),
    /// Instant at which the tooltip becomes visible (`anchor` time + delay).
    pub show_at: Instant,
    /// Whether the delay has elapsed and the tooltip is being painted.
    pub visible: bool,
}

/// Persistent parley shaping context for tooltip labels — created once so
/// system fonts are not reloaded every frame.
pub struct TooltipShaper {
    font_cx: FontContext,
    layout_cx: LayoutContext<()>,
}

impl Default for TooltipShaper {
    fn default() -> Self {
        Self {
            font_cx: FontContext::default(),
            layout_cx: LayoutContext::new(),
        }
    }
}

impl TooltipShaper {
    /// Paint `tip` into `scene`. `scale` is the HiDPI factor; `viewport` is the
    /// physical window size in pixels (used to keep the tooltip on screen).
    ///
    /// All geometry is computed in logical pixels and mapped to physical pixels
    /// by a single `Affine::scale(scale)` transform, matching how `paint_scene`
    /// renders the DOM.
    pub fn paint(
        &mut self,
        scene: &mut impl PaintScene,
        tip: &Tooltip,
        scale: f64,
        viewport: (u32, u32),
    ) {
        if tip.text.is_empty() {
            return;
        }

        // ── Shape the label (logical px) ─────────────────────────────────────
        let mut builder = self
            .layout_cx
            .ranged_builder(&mut self.font_cx, &tip.text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(FONT_SIZE));
        builder.push_default(StyleProperty::FontStack(FontStack::Single(
            FontFamily::Generic(GenericFamily::SansSerif),
        )));
        let mut layout = builder.build(&tip.text);
        layout.break_all_lines(Some(MAX_WIDTH));
        let text_w = layout.width() as f64;
        let text_h = layout.height() as f64;

        // ── Box geometry in logical px, clamped to the viewport ──────────────
        let vw = viewport.0 as f64 / scale;
        let vh = viewport.1 as f64 / scale;
        let box_w = text_w + 2.0 * PAD_X;
        let box_h = text_h + 2.0 * PAD_Y;
        let mut x = tip.anchor.0 as f64 + CURSOR_DX;
        let mut y = tip.anchor.1 as f64 + CURSOR_DY;
        if x + box_w > vw - EDGE {
            x = (vw - box_w - EDGE).max(EDGE);
        }
        if y + box_h > vh - EDGE {
            // Not enough room below the cursor — flip above it.
            y = (tip.anchor.1 as f64 - box_h - 8.0).max(EDGE);
        }

        let t = Affine::scale(scale); // logical → physical

        // ── Shadow + rounded-rect background ─────────────────────────────────
        scene.draw_box_shadow(
            t,
            Rect::new(x, y, x + box_w, y + box_h),
            SHADOW,
            RADIUS,
            4.0,
        );
        scene.fill(
            Fill::NonZero,
            t,
            &Paint::from(BG),
            None,
            &RoundedRect::new(x, y, x + box_w, y + box_h, RADIUS),
        );

        // ── Glyphs (mirrors blitz-paint's draw_glyphs bridge) ────────────────
        let glyph_t = t * Affine::translate((x + PAD_X, y + PAD_Y));
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(grun) = item else {
                    continue;
                };
                let mut gx = grun.offset();
                let gy = grun.baseline();
                let run = grun.run();
                let font = run.font();
                let font_size = run.font_size();
                let glyph_xform = run
                    .synthesis()
                    .skew()
                    .map(|a| Affine::skew(a.to_radians().tan() as f64, 0.0));
                scene.draw_glyphs(
                    font,
                    font_size,
                    true,
                    run.normalized_coords(),
                    Fill::NonZero,
                    &Paint::from(FG),
                    1.0,
                    glyph_t,
                    glyph_xform,
                    grun.glyphs().map(move |g| {
                        let out = Glyph {
                            id: g.id as _,
                            x: gx + g.x,
                            y: gy - g.y,
                        };
                        gx += g.advance;
                        out
                    }),
                );
            }
        }
    }
}
