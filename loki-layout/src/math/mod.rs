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
//! fractions (`mfrac`), scripts (`msup`/`msub`/`msubsup`), and radicals
//! (`msqrt`/`mroot`) — the same set the OOXML/ODF importers produce. Unknown
//! elements lay out their children as a row. This is a first-pass typesetter:
//! it does not stretch radicals/delimiters, balance spacing per the full TeX
//! `mathspacing` table, or handle matrices/n-ary operators.

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

#[cfg(test)]
#[path = "math_tests.rs"]
mod tests;
