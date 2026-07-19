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
//! **Scope.** The float is planned for the *anchoring paragraph*; the band is
//! passed to the layout as a [`crate::para::WrapBand`] and, on the paint path,
//! the body is laid out in two passes (`para_band`) so lines beside the float
//! are narrowed while lines below it reclaim the full column. When the float is
//! taller than its anchoring paragraph, the remaining extent is recorded as an
//! [`ActiveFloat`] on the flow state so the *following* paragraphs continue to
//! wrap beside it until the float bottom is cleared (cross-paragraph wrap).
//! `Square`, `Tight`, and `Through` modes wrap on one side (the tight contour
//! is approximated by the bounding box). `wrapNone` never reserves space —
//! Word flows the text at full column width and the object floats over (or,
//! `behindDoc`, under) it; the caller emits those as overlays via
//! [`crate::flow_para`], not as bands. `TopAndBottom` and behind-text floats
//! fall through to the block-stacked image path. Cross-paragraph wrap is bounded to a single page
//! and to consecutive plain paragraphs; a table/list/rule (or page break) below
//! the float reserves its remaining height instead of wrapping. OOXML
//! `wp:anchor` wrap children; ODF `style:wrap`.

use loki_doc_model::content::float::{TextWrap, WrapSide};

use crate::geometry::LayoutRect;
use crate::items::{PositionedImage, PositionedItem};
use crate::resolve::{CollectedImage, emu_to_pt};

/// A float whose vertical extent reaches past its anchoring paragraph, so the
/// paragraphs that follow on the same page keep wrapping beside it.
///
/// Tracked on [`super::FlowState::active_float`]. Coordinates are
/// page-content-relative (the same space as [`super::FlowState::cursor_y`]).
pub(crate) struct ActiveFloat {
    /// Page-content-relative y of the float's bottom edge. Paragraphs starting
    /// above this are narrowed to clear the band; the first one at/below it ends
    /// the wrap.
    pub bottom_y: f32,
    /// Horizontal band width (points) following paragraphs must clear.
    pub inset: f32,
    /// `true` when the float is on the left (text shifts right); `false` when on
    /// the right (text narrows but does not shift).
    pub shift_text: bool,
}

/// Reserves any remaining vertical extent of an active float and clears it.
///
/// Called when the float can no longer be wrapped by following text — a
/// non-paragraph block (table/list/rule), the end of the block list, or a page
/// boundary. Advancing `cursor_y` to the float bottom keeps later content from
/// overlapping the image.
pub(crate) fn reserve_active_float(state: &mut super::FlowState) {
    if let Some(af) = state.active_float.take()
        && state.cursor_y < af.bottom_y
    {
        state.cursor_y = af.bottom_y;
    }
}

/// Default gap between a float and the wrapped text, in points (~0.13").
pub(super) const FLOAT_WRAP_GAP: f32 = 9.0;

/// A planned float placement for one paragraph.
pub(crate) struct FloatPlacement {
    /// Extra left indent (points) — non-zero when the float sits on the left.
    pub indent_start_delta: f32,
    /// Extra right indent (points) — non-zero when the float sits on the right.
    pub indent_end_delta: f32,
    /// The float's image item in paragraph-content-local coordinates (x measured
    /// from the content-area left edge, y from the paragraph top).
    pub item: PositionedItem,
    /// Float height in points. When it exceeds the anchoring paragraph's text,
    /// the overhang becomes an [`ActiveFloat`] so following paragraphs wrap
    /// beside it (and any unused tail is reserved before the next block).
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
        // Text boxes are planned by `flow_textbox` (they render a box, not a
        // picture); skip them here.
        if img.textbox.is_some() {
            return None;
        }
        let f = img.float?;
        // Side-wrapping modes flow text beside the object. `wrapNone` is NOT one:
        // Word reserves no space for it — the text flows the full column width and
        // the image overlaps (drawn over/under it). The caller handles `wrapNone`
        // as an overlay; a behind-text float never displaces text either.
        let side_wraps = matches!(
            f.wrap,
            TextWrap::Square | TextWrap::Tight | TextWrap::Through
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
