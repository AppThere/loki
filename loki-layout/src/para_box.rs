// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The paragraph's border and background-fill box, split out of `para.rs` for
//! the 300-line ceiling. One function, called once from `layout_paragraph`.

use crate::geometry::LayoutRect;
use crate::items::{PositionedBorderRect, PositionedItem};
use crate::para::ResolvedParaProps;

/// Prepends the paragraph's border and background-fill rects to `items` (so
/// they render beneath the text). The box spans the **content column** — from
/// the start indent to the end indent, for the paragraph's full height —
/// matching Word, where a paragraph border/shading fills the column rather than
/// hugging the text ink. `available_width` is the paragraph's available width
/// (before indents). Background is inserted last so it sits behind the border.
pub(super) fn prepend_para_box(
    items: &mut Vec<PositionedItem>,
    para_props: &ResolvedParaProps,
    available_width: f32,
    height: f32,
) {
    let x = para_props.indent_start;
    let w = (available_width - para_props.indent_start - para_props.indent_end).max(0.0);
    let has_border = para_props.border_top.is_some()
        || para_props.border_right.is_some()
        || para_props.border_bottom.is_some()
        || para_props.border_left.is_some();
    if has_border {
        items.insert(
            0,
            PositionedItem::BorderRect(PositionedBorderRect {
                rect: LayoutRect::new(x, 0.0, w, height),
                top: para_props.border_top,
                right: para_props.border_right,
                bottom: para_props.border_bottom,
                left: para_props.border_left,
            }),
        );
    }
    // A `w:shd` texture paints as a hatch (bg + lines); a solid fill as a flat
    // rect (`para_background_item`).
    let bg_rect = LayoutRect::new(x, 0.0, w, height);
    if let Some(item) = crate::resolve::para_background_item(para_props, bg_rect) {
        items.insert(0, item);
    }
}
