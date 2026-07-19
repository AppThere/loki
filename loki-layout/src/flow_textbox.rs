// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Floating text-box (`wps` shape) placement for the flow engine.
//!
//! A [`CollectedImage`] whose [`textbox`](crate::resolve::CollectedImage::textbox)
//! is `Some` is a bordered/filled box carrying block content. This module flows
//! that content in a nested (`Pageless`) sub-layout, wraps it in a fill + border
//! group clipped to the box, and returns a [`FloatPlacement`] so the anchoring
//! paragraph reserves a side band and the surrounding text wraps around it —
//! reusing the same wrap machinery as a floating image.

use loki_doc_model::content::float::{TextWrap, WrapSide};

use super::float_impl::{FLOAT_WRAP_GAP, FloatPlacement};
use super::table_autofit::cell_flow_state;
use super::{FlowState, flow_block};
use crate::color::LayoutColor;
use crate::geometry::LayoutRect;
use crate::items::{BorderEdge, BorderStyle, PositionedBorderRect, PositionedItem, PositionedRect};
use crate::resolve::{CollectedImage, emu_to_pt};

/// Interior padding (points) between the box edge and its content — Word's
/// default `bodyPr` insets are 0.1"/0.05"; ~6 pt reads well at this scale.
const BOX_PAD: f32 = 6.0;

/// Parse a `"RRGGBB"` hex string into a [`LayoutColor`].
fn hex_to_color(s: &str) -> Option<LayoutColor> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 {
        return None;
    }
    let ch = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
    Some(LayoutColor::new(
        f32::from(ch(0)?) / 255.0,
        f32::from(ch(2)?) / 255.0,
        f32::from(ch(4)?) / 255.0,
        1.0,
    ))
}

/// Plan the first side-wrapping floating **text box** in `images`.
///
/// Returns its index (so the caller drops it from the block-stacked set) and a
/// [`FloatPlacement`] whose item is a clipped fill/border/content group. Returns
/// `None` when there is no wrapping text box or it would leave too little text.
pub(super) fn plan_textbox(
    state: &mut FlowState,
    images: &[CollectedImage],
    content_width: f32,
) -> Option<(usize, FloatPlacement)> {
    let idx = images.iter().position(|img| {
        img.textbox.is_some()
            && img.float.is_some_and(|f| {
                !f.behind_text
                    && matches!(
                        f.wrap,
                        TextWrap::Square | TextWrap::Tight | TextWrap::Through
                    )
            })
    })?;
    let img = &images[idx];
    let tb = img.textbox.as_ref()?;
    let fw = img.float?;

    let w = emu_to_pt(img.cx_emu);
    if w <= 0.0 {
        return None;
    }
    let band = w + FLOAT_WRAP_GAP;
    if band >= content_width * 0.75 {
        return None;
    }

    // WrapSide names the side TEXT occupies, so the box sits opposite (see
    // `plan_float`). Both/Largest default to a right float in a pull-quote.
    let float_left = matches!(fw.side, WrapSide::Right);
    let (indent_start_delta, indent_end_delta, x) = if float_left {
        (band, 0.0, 0.0)
    } else {
        (0.0, band, content_width - w)
    };

    // Flow the interior blocks in a nested Pageless sub-layout at the inner width.
    let inner_w = (w - 2.0 * BOX_PAD).max(1.0);
    let (inner_items, inner_h) = {
        let mut nested = cell_flow_state(
            state.resources,
            state.catalog,
            state.display_scale,
            state.options,
            inner_w,
            0.0,
            true,
            None,
        );
        for block in &tb.blocks {
            flow_block(&mut nested, block, 0);
        }
        (nested.current_items, nested.cursor_y)
    };

    // The box grows to fit its content if the authored height is too short.
    let authored_h = emu_to_pt(img.cy_emu);
    let box_h = authored_h.max(inner_h + 2.0 * BOX_PAD).max(1.0);
    let rect = LayoutRect::new(x, 0.0, w, box_h);

    let mut items: Vec<PositionedItem> = Vec::new();
    if let Some(fill) = tb.fill.as_deref().and_then(hex_to_color) {
        items.push(PositionedItem::FilledRect(PositionedRect {
            rect,
            color: fill,
        }));
    }
    for mut it in inner_items {
        it.translate(x + BOX_PAD, BOX_PAD);
        items.push(it);
    }
    if let Some(line) = tb.line.as_deref().and_then(hex_to_color) {
        let edge = Some(BorderEdge {
            color: line,
            width: 1.0,
            style: BorderStyle::Solid,
        });
        items.push(PositionedItem::BorderRect(PositionedBorderRect {
            rect,
            top: edge,
            right: edge,
            bottom: edge,
            left: edge,
        }));
    }

    Some((
        idx,
        FloatPlacement {
            indent_start_delta,
            indent_end_delta,
            item: PositionedItem::ClippedGroup {
                clip_rect: rect,
                items,
            },
            height: box_h,
        },
    ))
}
