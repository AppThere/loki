// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Vertical-stack constructs for the math typesetter (5.8): fenced
//! expressions with explicit fence attributes (`mfenced`), under/over scripts
//! and accents (`munder`/`mover`/`munderover`), and matrices (`mtable`).

use crate::color::LayoutColor;
use crate::font::FontResources;

use super::parse::MNode;
use super::{MBox, SCRIPT_SCALE, compose, layout_node, opt_node, row, shape};

/// Lays out `<mfenced>` honouring its `open`/`close`/`separators` attributes
/// (defaults `(`, `)`, `,` per MathML). The i-th separator character sits
/// between children i and i+1; the last separator repeats. Empty fence
/// strings suppress that side.
pub(super) fn fenced(
    resources: &mut FontResources,
    node: &MNode,
    font_size: f32,
    color: LayoutColor,
    scale: f32,
) -> MBox {
    let open = node.attr("open").unwrap_or("(");
    let close = node.attr("close").unwrap_or(")");
    let seps: Vec<char> = node
        .attr("separators")
        .unwrap_or(",")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let gap = font_size * 0.08;
    let mut boxes: Vec<(MBox, f32)> = Vec::new();
    for (i, child) in node.children.iter().enumerate() {
        if i > 0
            && let Some(&sep) = seps.get(i - 1).or(seps.last())
        {
            let b = shape::shape_token(resources, &sep.to_string(), font_size, false, color, scale);
            boxes.push((b, gap));
        }
        let b = layout_node(resources, child, font_size, color, scale);
        boxes.push((b, if i > 0 { gap } else { 0.0 }));
    }
    let content = compose::hbox(boxes);
    if open.is_empty() && close.is_empty() {
        return content;
    }
    compose::delimiters(resources, open, content, close, font_size, color, scale)
}

/// Lays out `munder` / `mover` / `munderover`: the base centred with its
/// limits (at script size) stacked below/above on a shared vertical axis.
/// An accent (`mover accent="true"`) hugs the base with a minimal gap.
pub(super) fn under_over(
    resources: &mut FontResources,
    node: &MNode,
    font_size: f32,
    color: LayoutColor,
    scale: f32,
) -> MBox {
    let small = font_size * SCRIPT_SCALE;
    let base = opt_node(resources, node.children.first(), font_size, color, scale);
    let (under, over) = match node.tag.as_str() {
        "munder" => (node.children.get(1), None),
        "mover" => (None, node.children.get(1)),
        _ => (node.children.get(1), node.children.get(2)),
    };
    let under = under.map(|c| layout_node(resources, c, small, color, scale));
    let over = over.map(|c| layout_node(resources, c, small, color, scale));
    let gap = if node.attr("accent") == Some("true") {
        font_size * 0.02
    } else {
        font_size * 0.12
    };

    let width = base
        .width
        .max(under.as_ref().map_or(0.0, |b| b.width))
        .max(over.as_ref().map_or(0.0, |b| b.width));
    let mut out = MBox::empty();
    out.width = width;

    let mut base = base;
    base.translate((width - base.width) / 2.0, 0.0);
    out.ascent = base.ascent;
    out.descent = base.descent;

    if let Some(mut b) = over {
        // Place the over box so its bottom sits `gap` above the base's top.
        b.translate((width - b.width) / 2.0, 0.0);
        b.shift_v(-(out.ascent + gap + b.descent));
        out.ascent = b.ascent; // shift_v updated it to the new box top
        out.items.extend(std::mem::take(&mut b.items));
    }
    if let Some(mut b) = under {
        // Place the under box so its top sits `gap` below the base's bottom.
        b.translate((width - b.width) / 2.0, 0.0);
        b.shift_v(out.descent + gap + b.ascent);
        out.descent = b.descent;
        out.items.extend(std::mem::take(&mut b.items));
    }
    out.items.extend(base.items);
    out
}

/// Lays out `<mtable>` as a grid: column widths and per-row heights are the
/// cell maxima, cells are centred in their column on the row baseline, and
/// the whole table is vertically centred on the math axis.
pub(super) fn table(
    resources: &mut FontResources,
    node: &MNode,
    font_size: f32,
    color: LayoutColor,
    scale: f32,
) -> MBox {
    let rows: Vec<Vec<MBox>> = node
        .children
        .iter()
        .filter(|r| r.tag == "mtr")
        .map(|r| {
            r.children
                .iter()
                .filter(|c| c.tag == "mtd")
                .map(|c| row(resources, &c.children, font_size, color, scale))
                .collect()
        })
        .collect();
    let ncols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if ncols == 0 {
        return MBox::empty();
    }
    let mut col_w = vec![0.0f32; ncols];
    for r in &rows {
        for (j, cell) in r.iter().enumerate() {
            col_w[j] = col_w[j].max(cell.width);
        }
    }
    let heights: Vec<(f32, f32)> = rows
        .iter()
        .map(|r| {
            r.iter().fold((0.0f32, 0.0f32), |(a, d), c| {
                (a.max(c.ascent), d.max(c.descent))
            })
        })
        .collect();
    let col_gap = font_size * 0.55;
    let row_gap = font_size * 0.35;
    let total_w = col_w.iter().sum::<f32>() + col_gap * (ncols - 1) as f32;
    let total_h = heights.iter().map(|(a, d)| a + d).sum::<f32>()
        + row_gap * heights.len().saturating_sub(1) as f32;
    // Centre the grid on the math axis so a matrix sits balanced beside text.
    let axis = font_size * 0.25;
    let ascent = total_h / 2.0 + axis;

    let mut out = MBox {
        width: total_w,
        ascent,
        descent: total_h - ascent,
        items: Vec::new(),
    };
    let mut y = -ascent;
    for (r, (row_asc, row_desc)) in rows.into_iter().zip(heights) {
        let baseline = y + row_asc;
        let mut x = 0.0f32;
        for (j, mut cell) in r.into_iter().enumerate() {
            cell.translate(x + (col_w[j] - cell.width) / 2.0, baseline);
            out.items.extend(cell.items);
            x += col_w[j] + col_gap;
        }
        y = baseline + row_desc + row_gap;
    }
    out
}
