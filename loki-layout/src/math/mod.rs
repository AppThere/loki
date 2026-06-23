// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! A small math typesetter that lays out MathML into positioned draw items.
//!
//! The document model stores math as a MathML string
//! (`loki_doc_model::content::inline::Inline::Math`). [`layout_math`] parses it
//! and produces a [`MathRender`] — a box of [`PositionedItem`]s (shaped glyph
//! runs plus fraction/radical rules) with intrinsic metrics — which
//! [`crate::para`] places inline via a Parley inline box.
//!
//! # Covered constructs
//!
//! Identifiers/numbers/operators (`mi`/`mn`/`mo`/`mtext`), rows (`mrow`),
//! fractions (`mfrac`), scripts (`msup`/`msub`/`msubsup`), radicals
//! (`msqrt`/`mroot`), and fenced expressions (`mfenced`, or a row wrapped in
//! matching fence operators). Radical signs and delimiters **stretch** to their
//! content via uniform glyph scaling (an approximation of true extensible
//! glyphs — the sign also widens). Unknown elements lay out their children as a
//! row. This is still a first-pass typesetter: it does not balance spacing per
//! the full TeX `mathspacing` table, or handle matrices / n-ary operators.

mod compose;
mod parse;
mod shape;

use crate::color::LayoutColor;
use crate::font::FontResources;
use crate::items::PositionedItem;
use parse::{MNode, parse_mathml};

/// A laid-out math expression in its own coordinate box.
///
/// `items` are positioned relative to the box's top-left corner `(0, 0)`; the
/// expression's baseline sits at `y = ascent`. `width`/`ascent`/`descent` give
/// the box metrics so the caller can place it inline on the text baseline.
pub(crate) struct MathRender {
    /// Total advance width in points.
    pub width: f32,
    /// Height above the baseline in points.
    pub ascent: f32,
    /// Depth below the baseline in points.
    pub descent: f32,
    /// Draw items, relative to the box top-left (`baseline = ascent`).
    pub items: Vec<PositionedItem>,
}

/// Lays out a MathML string at `font_size`, returning a [`MathRender`].
///
/// Returns an empty render (zero width, no items) when the string has no
/// usable `math` content.
pub(crate) fn layout_math(
    resources: &mut FontResources,
    mathml: &str,
    font_size: f32,
    color: LayoutColor,
    display_scale: f32,
) -> MathRender {
    let Some(root) = parse_mathml(mathml) else {
        return MathRender {
            width: 0.0,
            ascent: 0.0,
            descent: 0.0,
            items: Vec::new(),
        };
    };
    let mut mb = layout_node(resources, &root, font_size, color, display_scale);
    // Shift the baseline (currently y = 0) down to y = ascent so the box uses
    // top-left origin coordinates.
    let dy = mb.ascent;
    mb.translate(0.0, dy);
    MathRender {
        width: mb.width,
        ascent: mb.ascent,
        descent: mb.descent,
        items: mb.items,
    }
}

/// A baseline-relative math box: draw items with `y = 0` on the baseline,
/// positive `ascent` above and positive `descent` below.
pub(super) struct MBox {
    pub width: f32,
    pub ascent: f32,
    pub descent: f32,
    pub items: Vec<PositionedItem>,
}

impl MBox {
    fn empty() -> Self {
        Self {
            width: 0.0,
            ascent: 0.0,
            descent: 0.0,
            items: Vec::new(),
        }
    }

    /// Translates every draw item by `(dx, dy)`.
    fn translate(&mut self, dx: f32, dy: f32) {
        for item in &mut self.items {
            item.translate(dx, dy);
        }
    }

    /// Shifts the box vertically by `dy` (positive = down), keeping its
    /// ascent/descent consistent with the new position relative to the baseline.
    fn shift_v(&mut self, dy: f32) {
        self.translate(0.0, dy);
        self.ascent -= dy;
        self.descent += dy;
    }
}

/// Scale factor applied to script (super/sub/index) and nested content.
const SCRIPT_SCALE: f32 = 0.7;

/// Lays out one MathML node into a baseline-relative [`MBox`].
fn layout_node(
    resources: &mut FontResources,
    node: &MNode,
    font_size: f32,
    color: LayoutColor,
    scale: f32,
) -> MBox {
    let child = |i: usize| node.children.get(i);
    let small = font_size * SCRIPT_SCALE;
    match node.tag.as_str() {
        "mi" => shape::shape_token(
            resources,
            &node.text,
            font_size,
            is_italic_identifier(&node.text),
            color,
            scale,
        ),
        "mn" | "mtext" | "mo" | "ms" => {
            shape::shape_token(resources, &node.text, font_size, false, color, scale)
        }
        "mfrac" => {
            let num = opt_node(resources, child(0), font_size, color, scale);
            let den = opt_node(resources, child(1), font_size, color, scale);
            compose::frac(num, den, font_size, color)
        }
        "msup" => {
            let base = opt_node(resources, child(0), font_size, color, scale);
            let sup = child(1).map(|c| layout_node(resources, c, small, color, scale));
            compose::scripts(base, sup, None, font_size)
        }
        "msub" => {
            let base = opt_node(resources, child(0), font_size, color, scale);
            let sub = child(1).map(|c| layout_node(resources, c, small, color, scale));
            compose::scripts(base, None, sub, font_size)
        }
        "msubsup" => {
            let base = opt_node(resources, child(0), font_size, color, scale);
            let sub = child(1).map(|c| layout_node(resources, c, small, color, scale));
            let sup = child(2).map(|c| layout_node(resources, c, small, color, scale));
            compose::scripts(base, sup, sub, font_size)
        }
        "msqrt" => {
            let radicand = row(resources, &node.children, font_size, color, scale);
            compose::radical(resources, radicand, None, font_size, color, scale)
        }
        "mroot" => {
            let radicand = opt_node(resources, child(0), font_size, color, scale);
            let index = child(1).map(|c| layout_node(resources, c, small, color, scale));
            compose::radical(resources, radicand, index, font_size, color, scale)
        }
        // A fenced expression: lay out the children, then wrap in stretchy
        // delimiters. `<mfenced>` open/close attributes are not retained by the
        // parser, so the default parentheses are used.
        "mfenced" => {
            let content = row(resources, &node.children, font_size, color, scale);
            compose::delimiters(resources, "(", content, ")", font_size, color, scale)
        }
        // math, mrow, mstyle, semantics, mpadded, and unknowns: lay out children
        // as a horizontal row.
        _ => row(resources, &node.children, font_size, color, scale),
    }
}

/// Lays out `child` if present, else an empty box.
fn opt_node(
    resources: &mut FontResources,
    child: Option<&MNode>,
    font_size: f32,
    color: LayoutColor,
    scale: f32,
) -> MBox {
    match child {
        Some(c) => layout_node(resources, c, font_size, color, scale),
        None => MBox::empty(),
    }
}

/// Lays out a sequence of nodes as a horizontal row, adding operator spacing
/// around `<mo>` atoms.
fn row(
    resources: &mut FontResources,
    nodes: &[MNode],
    font_size: f32,
    color: LayoutColor,
    scale: f32,
) -> MBox {
    // Stretchy delimiters: a row that opens and closes with matching fence
    // operators is laid out as its inner content wrapped in delimiters scaled to
    // that content's height.
    if nodes.len() >= 2 {
        let (first, last) = (&nodes[0], &nodes[nodes.len() - 1]);
        if first.tag == "mo"
            && last.tag == "mo"
            && is_open_fence(&first.text)
            && is_close_fence(&last.text)
        {
            let content = row(
                resources,
                &nodes[1..nodes.len() - 1],
                font_size,
                color,
                scale,
            );
            return compose::delimiters(
                resources,
                &first.text,
                content,
                &last.text,
                font_size,
                color,
                scale,
            );
        }
    }

    let op_gap = font_size * 0.17;
    let mut boxes: Vec<(MBox, f32)> = Vec::new();
    let mut prev_mo = false;
    for node in nodes {
        let b = layout_node(resources, node, font_size, color, scale);
        if b.width == 0.0 && b.items.is_empty() {
            continue;
        }
        let is_mo = node.tag == "mo";
        let gap = if (is_mo || prev_mo) && !boxes.is_empty() {
            op_gap
        } else {
            0.0
        };
        boxes.push((b, gap));
        prev_mo = is_mo;
    }
    compose::hbox(boxes)
}

/// MathML convention: a single-letter identifier is rendered italic, a
/// multi-letter one (e.g. a function name like `sin`) upright.
fn is_italic_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if c.is_alphabetic())
}

/// Whether `s` is a single opening fence character (paren/bracket/brace/bar or
/// the floor/ceiling/angle openers) that should stretch to its content.
fn is_open_fence(s: &str) -> bool {
    matches!(
        s,
        "(" | "[" | "{" | "|" | "\u{2016}" | "\u{230A}" | "\u{2308}" | "\u{27E8}" | "\u{2329}"
    )
}

/// Whether `s` is a single closing fence character.
fn is_close_fence(s: &str) -> bool {
    matches!(
        s,
        ")" | "]" | "}" | "|" | "\u{2016}" | "\u{230B}" | "\u{2309}" | "\u{27E9}" | "\u{232A}"
    )
}

#[cfg(test)]
#[path = "math_tests.rs"]
mod tests;
