// SPDX-License-Identifier: Apache-2.0

//! Paginated tile-click handling, extracted from `editor_canvas.rs`.
//!
//! The click resolves against the editor's own paginated layout rather than the
//! renderer's: `DocumentView` resolves reflow clicks itself (it owns that
//! layout), but in paginated mode the editor is the single canonical layout, so
//! the tile forwards raw tile-local coordinates and the hit test happens here.

use std::sync::{Arc, Mutex};

use dioxus::prelude::*;
use loki_doc_model::loro_bridge::derive_loro_cursor;

use crate::editing::cursor::CursorState;
use crate::editing::hit_test::{hit_test_page, link_at_point, open_or_run};
use crate::editing::state::DocumentState;

/// Everything a paginated tile click needs, so the call site stays one line.
pub(super) struct TileClickCtx {
    pub(super) doc_state: Arc<Mutex<DocumentState>>,
    pub(super) loro_doc: Signal<Option<loro::LoroDoc>>,
    pub(super) cursor_state: Signal<CursorState>,
    pub(super) macro_run_request: Signal<Option<String>>,
}

/// Places the caret at a paginated tile click, or opens a hyperlink when the
/// Ctrl/Cmd modifier was held.
pub(super) fn on_tile_click(
    ctx: &mut TileClickCtx,
    page_index: usize,
    x_pt: f32,
    y_pt: f32,
    open_link: bool,
) {
    let layout_opt = {
        let Ok(state) = ctx.doc_state.lock() else {
            return;
        };
        state.paginated_layout.clone()
    };
    let Some(layout) = layout_opt else { return };
    if open_link && let Some(url) = link_at_point(&layout, page_index, x_pt, y_pt) {
        open_or_run(&url, ctx.macro_run_request);
        return; // Ctrl/Cmd+click hit a link/button; no caret move.
    }
    let Some(pos) = hit_test_page(page_index, x_pt, y_pt, &layout) else {
        return;
    };
    let loro_cursor = ctx
        .loro_doc
        .read()
        .as_ref()
        .and_then(|ldoc| derive_loro_cursor(ldoc, pos.paragraph_index, pos.byte_offset));
    let mut cs = ctx.cursor_state.write();
    cs.loro_cursor = loro_cursor;
    cs.anchor = Some(pos.clone());
    cs.focus = Some(pos);
}
