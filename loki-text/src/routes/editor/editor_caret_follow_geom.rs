// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Resolving the caret's content-space rect for whichever renderer is active.
//!
//! Extracted from `editor_caret_follow` so neither file approaches the 300-line
//! ceiling — that module is mostly the trigger rule and the reasoning behind it
//! (see its docs for I-20), and this is the geometry lookup, which is a
//! separable concern with a different reason to change.
//!
//! Both branches end in `editing::caret_reveal`, which owns the arithmetic; the
//! work here is getting the right layout and the right scale factor for the
//! current mode.

use std::sync::{Arc, Mutex};

use loki_renderer::ViewMode;
use loki_renderer::render_layout::{reflow_layout_content_width_pt, reflow_type_scale};

use crate::editing::caret_reveal::{PageStack, caret_rect_paginated, caret_rect_reflow};
use crate::editing::cursor::DocumentPosition;
use crate::editing::state::{DocumentState, ensure_reflow_layout};

/// Everything the resolution needs that is not the caret itself.
#[derive(Clone, Copy)]
pub(super) struct CaretGeometryInput {
    /// Active renderer.
    pub(super) view_mode: ViewMode,
    /// Measured visible width of the scroll container, in CSS px. Drives the
    /// reflow layout width and its responsive type scale.
    pub(super) client_width: f32,
    /// Zoom fraction (1.0 = 100%); paginated only.
    pub(super) zoom: f32,
    /// Inter-page gap in CSS px; paginated only.
    pub(super) page_gap_px: f32,
    /// Scroll container top padding, in CSS px.
    pub(super) content_top_px: f32,
}

/// The caret's rect in scroll-container content coordinates, or `None` when the
/// geometry cannot be resolved yet.
///
/// `None` is a normal state, not an error: the layout may not have been
/// recomputed since the last edit, or the container may not be measured. The
/// caller treats it as "nothing to reveal" and retries on the next caret move,
/// rather than scrolling to a guess.
pub(super) fn caret_content_rect(
    doc_state: &Arc<Mutex<DocumentState>>,
    focus: &DocumentPosition,
    input: CaretGeometryInput,
) -> Option<(f32, f32, f32, f32)> {
    if input.view_mode == ViewMode::Reflow {
        if input.client_width <= 1.0 {
            return None;
        }
        let scale = reflow_type_scale(input.client_width);
        let content_w = reflow_layout_content_width_pt(input.client_width);
        let layout = ensure_reflow_layout(doc_state, content_w)?;
        return caret_rect_reflow(
            &layout,
            focus.paragraph_index,
            focus.byte_offset,
            scale,
            input.content_top_px,
        );
    }

    // One lock for both reads: taking it twice could observe a layout and a
    // page height from either side of a relayout, which is how a caret lands
    // one page off.
    let (layout, page_height_px) = {
        let state = doc_state.lock().ok()?;
        (state.paginated_layout.clone()?, state.page_height_px)
    };
    let stack = PageStack {
        page_height_px,
        page_gap_px: input.page_gap_px,
        zoom: input.zoom,
        content_top_px: input.content_top_px,
    };
    caret_rect_paginated(&layout, focus, stack)
}
