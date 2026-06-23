// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Box composition for the math typesetter.
//!
//! Combines the baseline-relative [`MBox`]es produced by token shaping into
//! the structural MathML constructs: horizontal rows, fractions, scripts, and
//! radicals. Spacing constants follow TeX-ish proportions of the current font
//! size; they are deliberately simple (this is a first-pass typesetter, not a
//! full TeX math engine).

use super::MBox;
use super::shape::shape_token;
use crate::color::LayoutColor;
use crate::font::FontResources;
use crate::geometry::LayoutRect;
use crate::items::{PositionedItem, PositionedRect};

/// Lays out `boxes` left-to-right on a shared baseline, inserting `gap` between
/// adjacent boxes. The result's ascent/descent are the maxima of its parts.
pub(super) fn hbox(boxes: Vec<(MBox, f32)>) -> MBox {
    let mut out = MBox::empty();
    let mut x = 0.0f32;
    for (i, (mut b, gap)) in boxes.into_iter().enumerate() {
        if i > 0 {
            x += gap;
        }
        b.translate(x, 0.0);
        out.ascent = out.ascent.max(b.ascent);
        out.descent = out.descent.max(b.descent);
        x += b.width;
        out.items.extend(b.items);
    }
    out.width = x;
    out
}

/// Stacks `num` over `den` separated by a fraction bar (`<mfrac>`). The bar is
/// centred on the math axis (≈ ¼ em above the baseline).
pub(super) fn frac(num: MBox, den: MBox, font_size: f32, color: LayoutColor) -> MBox {
    let rule = (font_size * 0.055).max(0.6);
    let axis = font_size * 0.25;
    let gap = font_size * 0.18;
    let pad = font_size * 0.15;
    let width = num.width.max(den.width) + 2.0 * pad;
    let bar_y = -axis;

    let mut num = num;
    let mut den = den;
    num.translate((width - num.width) / 2.0, bar_y - gap - num.descent);
    den.translate((width - den.width) / 2.0, bar_y + gap + den.ascent);

    let mut items = vec![PositionedItem::FilledRect(PositionedRect {
        rect: LayoutRect::new(0.0, bar_y - rule / 2.0, width, rule),
        color,
    })];
    let ascent = axis + gap + num.ascent + num.descent;
    let descent = (den.ascent + den.descent + gap - axis).max(0.0);
    items.extend(num.items);
    items.extend(den.items);
    MBox {
        width,
        ascent,
        descent,
        items,
    }
}

/// Attaches an optional super- and/or subscript to `base`
/// (`<msup>`/`<msub>`/`<msubsup>`). Scripts are placed to the right of the base.
pub(super) fn scripts(base: MBox, sup: Option<MBox>, sub: Option<MBox>, font_size: f32) -> MBox {
    let sup_shift = font_size * 0.45;
    let sub_shift = font_size * 0.18;
    let script_x = base.width;

    let mut width = base.width;
    let mut ascent = base.ascent;
    let mut descent = base.descent;
    let mut items = base.items;

    if let Some(mut s) = sup {
        s.translate(script_x, -sup_shift);
        ascent = ascent.max(sup_shift + s.ascent);
        width = width.max(script_x + s.width);
        items.extend(s.items);
    }
    if let Some(mut s) = sub {
        s.translate(script_x, sub_shift);
        descent = descent.max(sub_shift + s.descent);
        width = width.max(script_x + s.width);
        items.extend(s.items);
    }

    MBox {
        width,
        ascent,
        descent,
        items,
    }
}

/// Wraps `radicand` in a radical sign with an overbar (`<msqrt>`/`<mroot>`).
///
/// The surd is the `√` glyph shaped at the base font size (it does not stretch
/// to tall radicands — a documented approximation); the overbar is a rule drawn
/// across the top of the radicand. An optional `index` (for `<mroot>`) is placed
/// above the surd.
pub(super) fn radical(
    resources: &mut FontResources,
    radicand: MBox,
    index: Option<MBox>,
    font_size: f32,
    color: LayoutColor,
    display_scale: f32,
) -> MBox {
    let rule = (font_size * 0.055).max(0.6);
    let gap = font_size * 0.12;
    let surd = shape_token(
        resources,
        "\u{221A}",
        font_size,
        false,
        color,
        display_scale,
    );

    // Overbar spans the radicand (and a little lead-in), sitting `gap` above the
    // radicand's top edge.
    let bar_y = -(radicand.ascent + gap) - rule;
    let mut items: Vec<PositionedItem> = Vec::new();
    let mut x = 0.0f32;

    // Optional degree index, raised and to the left of the surd.
    if let Some(mut idx) = index {
        idx.translate(x, -(surd.ascent * 0.6));
        x += idx.width;
        items.extend(idx.items);
    }

    let mut surd = surd;
    surd.translate(x, 0.0);
    let surd_right = x + surd.width;
    items.extend(surd.items);

    let mut radicand = radicand;
    radicand.translate(surd_right, 0.0);
    let width = surd_right + radicand.width;
    items.extend(radicand.items);

    items.push(PositionedItem::FilledRect(PositionedRect {
        rect: LayoutRect::new(surd_right, bar_y, width - surd_right, rule),
        color,
    }));

    let ascent = (radicand.ascent + gap + rule).max(surd.ascent).max(-bar_y);
    let descent = radicand.descent.max(surd.descent);
    MBox {
        width,
        ascent,
        descent,
        items,
    }
}
