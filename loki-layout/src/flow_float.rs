// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Floating-image text wrap (gap #12).
//!
//! An anchored (floating) drawing sits beside the body text, which wraps around
//! it. This module plans the placement of such a float for the paragraph it is
//! anchored in: it chooses a side, reserves the horizontal band the text must
//! avoid (returned as left/right indent deltas), and produces the float's image
//! item.
//!
//! **Scope (v1).** Wrapping is applied to the *anchoring paragraph* only, using
//! a uniform indent (every line of that paragraph clears the float, matching the
//! same Parley per-line-width limitation documented for drop caps). `Square`,
//! `Tight`, `Through`, and non-behind `None` modes wrap on one side (the tight
//! contour is approximated by the bounding box; a margin-anchored `wrapNone`
//! image reserves its space in Word, so text flows beside rather than under it).
//! `TopAndBottom` and behind-text floats fall through to the block-stacked image
//! path. A float taller than its paragraph reserves its full height so following
//! paragraphs clear it, but they do not themselves wrap. OOXML `wp:anchor` wrap
//! children; ODF `style:wrap`.

use loki_doc_model::content::float::{TextWrap, WrapSide};

use crate::geometry::LayoutRect;
use crate::items::{PositionedImage, PositionedItem};
use crate::resolve::{CollectedImage, emu_to_pt};

/// Default gap between a float and the wrapped text, in points (~0.13").
const FLOAT_WRAP_GAP: f32 = 9.0;

/// A planned float placement for one paragraph.
pub(crate) struct FloatPlacement {
    /// Extra left indent (points) — non-zero when the float sits on the left.
    pub indent_start_delta: f32,
    /// Extra right indent (points) — non-zero when the float sits on the right.
    pub indent_end_delta: f32,
    /// The float's image item in paragraph-content-local coordinates (x measured
    /// from the content-area left edge, y from the paragraph top).
    pub item: PositionedItem,
    /// Float height in points; the paragraph reserves at least this much so the
    /// following paragraph clears the float.
    pub height: f32,
}

/// Plans wrapping for the first side-wrapping float in `images`.
///
/// Returns the float's index in `images` (so the caller can remove it from the
/// block-stacked set) and its [`FloatPlacement`]. Returns `None` when no image
/// is a side-wrapping float (`Square`/`Tight`/`Through`/non-behind `None`), or
/// when the float would leave too little usable text width.
pub(crate) fn plan_float(
    images: &[CollectedImage],
    content_width: f32,
) -> Option<(usize, FloatPlacement)> {
    let (idx, img, fw) = images.iter().enumerate().find_map(|(i, img)| {
        let f = img.float?;
        // Side-wrapping modes flow text beside the object. `None` is included
        // when the float is not behind the text: Word reserves space for a
        // margin-anchored `wrapNone` image (text flows beside, not under it),
        // matching the reference. A behind-text float never displaces text.
        let side_wraps = matches!(
            f.wrap,
            TextWrap::Square | TextWrap::Tight | TextWrap::Through | TextWrap::None
        );
        (side_wraps && !f.behind_text).then_some((i, img, f))
    })?;

    let w = emu_to_pt(img.cx_emu);
    let h = emu_to_pt(img.cy_emu);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let band = w + FLOAT_WRAP_GAP;
    // Leave at least a quarter of the column for text; otherwise skip wrapping.
    if band >= content_width * 0.75 {
        return None;
    }

    // WrapSide names the side TEXT occupies, so the float sits opposite:
    //   side=Right → text right → float LEFT;   side=Left → text left → float RIGHT.
    //   Both/Largest → default to a left float (text flows to its right).
    let float_left = !matches!(fw.side, WrapSide::Left);

    let (indent_start_delta, indent_end_delta, x) = if float_left {
        (band, 0.0, 0.0)
    } else {
        (0.0, band, content_width - w)
    };

    let item = PositionedItem::Image(PositionedImage {
        rect: LayoutRect::new(x, 0.0, w, h),
        src: img.src.clone(),
        alt: img.alt.clone(),
    });

    Some((
        idx,
        FloatPlacement {
            indent_start_delta,
            indent_end_delta,
            item,
            height: h,
        },
    ))
}

#[cfg(test)]
#[path = "flow_float_tests.rs"]
mod tests;
