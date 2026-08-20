// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Inline-image placement within a paragraph: block-stacking non-floating
//! images above the text and emitting `wrapNone` overlays.
//!
//! Extracted from `flow_para.rs` for the 300-line ceiling. Both functions
//! mutate a paragraph layout *after* shaping, which is why the caller must take
//! a private copy first — a shaped layout is shared with the paragraph cache
//! (Spec 09 S9-1), and mutating it in place would leak this paragraph's images
//! into every other placement of the same text.

use loki_doc_model::content::float::{TextWrap, WrapSide};

use crate::geometry::LayoutRect;
use crate::items::{PositionedImage, PositionedItem};
use crate::para::ParagraphLayout;
use crate::resolve::emu_to_pt;

/// Scales `(w, h)` down so the width fits `max_width`, **aspect preserved**.
///
/// Never scales *up*: an element narrower than the column keeps its own size.
/// A non-positive `max_width` or width returns the input unchanged — there is no
/// column to fit to, and inventing one would resize an element on the strength
/// of a measurement that had not arrived.
///
/// # Why this exists, and why it is reflow-only
///
/// Spec 08 T7.3: *the document never scrolls horizontally*. An image wider than
/// the reading column used to widen the reflow tile instead, so the whole
/// document scrolled sideways to reach it — the defect T7.3 names.
///
/// It must **not** apply to the paginated view. There an oversized image is a
/// fidelity question rather than a reading one: the page is a fixed physical
/// size, Word and LibreOffice paint the image at its declared dimensions and let
/// it overhang, and silently shrinking it would make our page disagree with
/// theirs on screen, on export and in print.
#[must_use]
pub(crate) fn fit_to_column(w: f32, h: f32, max_width: f32) -> (f32, f32) {
    if w <= 0.0 || max_width <= 0.0 || w <= max_width {
        return (w, h);
    }
    let scale = max_width / w;
    (max_width, h * scale)
}

/// Block-stacks a paragraph's non-floating images above its text (gap #9) and
/// returns any `wrapNone` overlays for the caller to emit after floats.
///
/// `fit_oversized` is the reflow view's "never scroll sideways" rule (T7.3):
/// when set, an image wider than `content_width` is scaled to it with its aspect
/// preserved. See [`fit_to_column`] for why the paginated view must not.
///
/// `alignment` is the paragraph's own (`w:jc`): a block image is laid out in
/// the text column and follows it, so a centred figure paragraph centres its
/// picture. Every image used to be pinned to `x = 0`, which put `acid2-docx`'s
/// centred chart hard against the left margin.
///
/// TODO(inline-image-flow): Parley has no inline image boxes, so images are a
/// block-level prefix — existing items shift down to make room. Shared by
/// [`flow_paragraph`] and the keep-with-next chain (`flow_para_chain`) so an
/// image in a `keepNext` paragraph (e.g. a captioned figure) is not dropped.
pub(crate) fn stack_block_images(
    para_layout: &mut ParagraphLayout,
    images: &[crate::resolve::CollectedImage],
    content_width: f32,
    fit_oversized: bool,
    alignment: parley::Alignment,
) -> Vec<(bool, PositionedItem)> {
    let mut total_image_height = 0.0f32;
    let mut image_items: Vec<PositionedItem> = Vec::new();
    // Overlay floats (`wrapNone`): Word reserves no space for them, so instead
    // of stacking above the text they float at a side-anchored position over
    // the full-width text (or under it when `behind_text`).
    let mut overlay_items: Vec<(bool, PositionedItem)> = Vec::new();
    for img in images {
        if img.cx_emu == 0 && img.cy_emu == 0 {
            continue; // zero-size image — skip without crashing
        }
        let (w, h) = {
            let natural = (emu_to_pt(img.cx_emu), emu_to_pt(img.cy_emu));
            if fit_oversized {
                fit_to_column(natural.0, natural.1, content_width)
            } else {
                natural
            }
        };
        if let Some(f) = img.float.filter(|f| f.wrap == TextWrap::None) {
            // Anchor to the same side `plan_float` would have chosen: text on
            // the left (`side=Left`) means the object sits on the right.
            let x = if matches!(f.side, WrapSide::Left) {
                (content_width - w).max(0.0)
            } else {
                0.0
            };
            overlay_items.push((
                f.behind_text,
                PositionedItem::Image(PositionedImage {
                    rect: LayoutRect::new(x, 0.0, w, h),
                    src: img.src.clone(),
                    alt: img.alt.clone(),
                }),
            ));
            continue;
        }
        // Justified behaves as start for a lone object, matching Word: there is
        // nothing to stretch between.
        let x = match alignment {
            parley::Alignment::Center => ((content_width - w) / 2.0).max(0.0),
            parley::Alignment::End => (content_width - w).max(0.0),
            _ => 0.0,
        };
        image_items.push(PositionedItem::Image(PositionedImage {
            rect: LayoutRect::new(x, total_image_height, w, h),
            src: img.src.clone(),
            alt: img.alt.clone(),
        }));
        total_image_height += h;
    }
    if total_image_height > 0.0 {
        // Expand background fill to cover image area (first item when present).
        if let Some(PositionedItem::FilledRect(bg)) = para_layout.items.first_mut() {
            bg.rect.size.height += total_image_height;
        }
        // Shift all existing paragraph items down by total image height.
        for item in &mut para_layout.items {
            item.translate(0.0, total_image_height);
        }
        para_layout.height += total_image_height;
        // Prepend image items (they render before paragraph text).
        image_items.append(&mut para_layout.items);
        para_layout.items = image_items;
    }
    overlay_items
}

/// Emits `wrapNone` overlay images: behind-text ones under the whole paragraph
/// (drawn first), in-front ones over the text (drawn last). Neither reserves
/// vertical space nor shifts the text.
pub(crate) fn apply_overlay_images(
    para_layout: &mut ParagraphLayout,
    overlay_items: Vec<(bool, PositionedItem)>,
) {
    for (behind, item) in overlay_items {
        if behind {
            para_layout.items.insert(0, item);
        } else {
            para_layout.items.push(item);
        }
    }
}

#[cfg(test)]
#[path = "flow_para_images_tests.rs"]
mod tests;
