// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The paragraph's border and background-fill box, split out of `para.rs` for
//! the 300-line ceiling. One function, called once from `layout_paragraph`.

use crate::geometry::LayoutRect;
use crate::items::{BorderEdge, PositionedBorderRect, PositionedItem};
use crate::para::ResolvedParaProps;

/// Vertical room the paragraph's own top and bottom borders occupy **outside**
/// its text, in points: each edge's `w:space` offset plus the rule's own width.
///
/// Word treats a paragraph border as part of the paragraph's extent rather than
/// as ink drawn over the surrounding spacing, so a blank paragraph carrying a
/// bottom border — Word's horizontal-rule idiom — is `space + width` tall
/// despite having no text, and the following block starts that much lower.
/// Dropping it made every rule in a document silently one rule too short:
/// measured against Word's own PDF for `iris-blueprint.docx`, whose 14 rules
/// all declare `w:sz="4" w:space="1"` (0.5 pt + 1 pt), the blank→heading
/// transition came out 1.58 pt tight against a predicted 1.5 pt.
fn border_extents(para_props: &ResolvedParaProps) -> (f32, f32) {
    let ext = |e: Option<BorderEdge>| e.map_or(0.0, |b| b.width + b.spacing);
    (ext(para_props.border_top), ext(para_props.border_bottom))
}

/// Insets `items` by the top border's extent, prepends the border/background
/// box sized to the whole outer box, and returns `(top_extent, outer_height)`.
///
/// The three paragraph-layout paths (empty, banded, and the main one) all go
/// through here so the extent is applied once and identically: a paragraph
/// whose box grew but whose baselines did not would print its text over its own
/// rule.
pub(super) fn apply_border_box(
    items: &mut Vec<PositionedItem>,
    para_props: &ResolvedParaProps,
    available_width: f32,
    content_height: f32,
) -> (f32, f32) {
    let (top, bottom) = border_extents(para_props);
    if top > 0.0 {
        for it in items.iter_mut() {
            it.translate(0.0, top);
        }
    }
    let outer = top + content_height + bottom;
    prepend_para_box(items, para_props, available_width, outer);
    (top, outer)
}

/// Shifts paragraph-local line boundaries down by `dy` (the top border extent).
pub(super) fn shift_boundaries(b: Vec<(f32, f32)>, dy: f32) -> Vec<(f32, f32)> {
    if dy == 0.0 {
        return b;
    }
    b.into_iter().map(|(a, z)| (a + dy, z + dy)).collect()
}

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
