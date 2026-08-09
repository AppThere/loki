// SPDX-License-Identifier: Apache-2.0

//! Renders the loaded-document arm of `editor_canvas.rs`'s `DocumentView`
//! match — extracted (CLAUDE.md's split technique 3, cohesive-cluster
//! extraction) because `editor_canvas.rs` is a baselined 300-line-ceiling
//! file (`scripts/file-ceiling-baseline.txt`) that must not grow, and this
//! arm is the single largest self-contained cluster in it: one `rsx!` tree
//! and the five `DocumentView` event-handler closures that go with it.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_app_shell::spell::SpellService;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::derive_loro_cursor;
use loki_renderer::{DocumentView, RendererCursorPos, TileContext, ViewMode};

use super::editor_canvas_metrics::{CANVAS_CONTENT_PADDING_PX, DEFAULT_VIEWPORT_HEIGHT_PX};
use super::editor_canvas_spell::open_spell_panel_at;
use super::editor_spell::SpellMenu;
use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::editing::hit_test::open_or_run;
use crate::editing::state::DocumentState;

/// Everything [`render_loaded_document`] needs beyond the document itself —
/// bundled for the same reason [`super::dom_reflow::EditorSignals`] is: one
/// parameter instead of a dozen at a call site in a file that must not grow.
pub(super) struct LoadedDocumentCtx<'a> {
    pub(super) doc_state_render: &'a Arc<Mutex<DocumentState>>,
    pub(super) doc_state_mousedown: &'a Arc<Mutex<DocumentState>>,
    pub(super) doc_state_context: &'a Arc<Mutex<DocumentState>>,
    pub(super) cursor_state: Signal<CursorState>,
    pub(super) loro_doc: Signal<Option<loro::LoroDoc>>,
    pub(super) macro_run_request: Signal<Option<String>>,
    pub(super) view_mode: Signal<ViewMode>,
    pub(super) zoom_percent: Signal<u32>,
    pub(super) scroll_metrics: Signal<super::editor_scrollbar::ScrollMetrics>,
    pub(super) scroll_offset: Signal<f32>,
    pub(super) service: &'a SpellService,
    pub(super) spell_menu: Signal<Option<SpellMenu>>,
}

/// The paginated/reflow `DocumentView`, once a document and its first
/// layout are ready — the body of `editor_canvas.rs`'s
/// `Some((loaded_path, Ok(doc))) if ...` match arm.
pub(super) fn render_loaded_document(doc: &Document, ctx: &LoadedDocumentCtx) -> Element {
    // Use the live post-mutation document from doc_state when available;
    // fall back to the original resource doc before seed_layout_from_document
    // has run. Read the matching paginated layout under the same lock so the
    // renderer can reuse it (single canonical layout) instead of recomputing.
    let (doc_opt, paginated_layout) = match ctx.doc_state_render.lock() {
        Ok(s) => (s.document.clone(), s.paginated_layout.clone()),
        Err(_) => (None, None),
    };
    let arc_doc = doc_opt.unwrap_or_else(|| Arc::new(doc.clone()));
    let (cursor_pos, selection_anchor) = {
        let cs = ctx.cursor_state.read();
        let to_renderer = |pos: &DocumentPosition| RendererCursorPos {
            page_index: pos.page_index,
            paragraph_index: pos.paragraph_index,
            byte_offset: pos.byte_offset,
        };
        (
            cs.focus.as_ref().map(to_renderer),
            cs.anchor.as_ref().map(to_renderer),
        )
    };
    let mut cursor_state = ctx.cursor_state;
    let loro_doc = ctx.loro_doc;
    let view_mode = ctx.view_mode;
    let macro_run_request = ctx.macro_run_request;
    let scroll_metrics = ctx.scroll_metrics;
    let scroll_offset = ctx.scroll_offset;
    let zoom_percent = ctx.zoom_percent;
    let spell_menu = ctx.spell_menu;
    // Cloned rather than captured by reference: these closures are `move` and
    // stored as event handlers past this function's own return, so they
    // cannot hold `ctx`'s borrow. Both are cheap `Arc` handles.
    let doc_state_mousedown = Arc::clone(ctx.doc_state_mousedown);
    let doc_state_context = Arc::clone(ctx.doc_state_context);
    let service = ctx.service.clone();
    rsx! {
        DocumentView {
            doc: arc_doc,
            paginated_layout,
            zoom: zoom_percent() as f64 / 100.0,
            // Real measured viewport height (falls back to a sensible
            // default before the first measure). Drives tile virtualization:
            // only pages within ~one screen of the viewport are GPU-rendered.
            viewport_height_px: {
                let h = scroll_metrics().client_height as f64;
                if h > 1.0 { h } else { DEFAULT_VIEWPORT_HEIGHT_PX }
            },
            // Real scroll offset so the renderer can virtualize tiles to the
            // viewport (this scroll container is the editor's, so the
            // position must be passed in).
            viewport_top_px: scroll_offset() as f64,
            cursor_pos,
            selection_anchor,
            view_mode: view_mode(),
            // Width for reflow layout; <= 0 until the canvas is measured
            // (mount rect or first scroll event).
            reflow_width_px: scroll_metrics().client_width as f64,
            // Design tokens injected so the render layer need not depend on
            // appthere_ui (Spec 01 audit A-8).
            page_gap_px: tokens::PAGE_GAP_PX as f64,
            content_padding_bottom_px: CANVAS_CONTENT_PADDING_PX,
            // The same fact the CSS above applies, reaching the residency
            // plan so it measures visibility from the origin the scroll
            // offset actually uses.
            content_padding_top_px: CANVAS_CONTENT_PADDING_PX,
            // Resident page-texture budget (Spec 08 T2.1), from the live
            // DeviceProfile, plus the display's device pixel ratio — the
            // renderer needs both to decide what to mount and at what
            // rasterisation scale, and neither is reachable from L4
            // (DeviceProfile is L5).
            texture_budget: crate::texture_budget::current(),
            device_scale_factor: crate::texture_budget::device_scale_factor(),
            // Paginated: hit-test against the editor's paginated layout
            // (reflow clicks arrive via on_reflow_click).
            on_tile_click: {
                let mut tctx = super::editor_canvas_click::TileClickCtx {
                    doc_state: doc_state_mousedown,
                    loro_doc,
                    cursor_state,
                    macro_run_request,
                };
                move |c: (usize, f32, f32, bool)| {
                    super::editor_canvas_click::on_tile_click(&mut tctx, c.0, c.1, c.2, c.3)
                }
            },
            // Reflow: DocumentView already resolved the click to a
            // (paragraph, byte) position in the continuous layout.
            on_reflow_click: move |(para, byte): (usize, usize)| {
                let loro_cursor = loro_doc
                    .read()
                    .as_ref()
                    .and_then(|ldoc| derive_loro_cursor(ldoc, para, byte));
                // page_index is meaningless in reflow (the caret is painted
                // from paragraph/byte); 0 is a placeholder.
                let pos = DocumentPosition::top_level(0, para, byte);
                let mut cs = cursor_state.write();
                cs.loro_cursor = loro_cursor;
                cs.anchor = Some(pos.clone());
                cs.focus = Some(pos);
            },
            // Reflow Ctrl/Cmd+click on a link (URL already resolved).
            on_open_link: move |url: String| {
                open_or_run(&url, macro_run_request);
            },
            // Reflow drag-select: move only the focus, keeping the anchor so
            // a range selection grows under the pointer.
            on_reflow_drag: move |(para, byte): (usize, usize)| {
                let loro_cursor = loro_doc
                    .read()
                    .as_ref()
                    .and_then(|ldoc| derive_loro_cursor(ldoc, para, byte));
                let mut cs = cursor_state.write();
                cs.loro_cursor = loro_cursor;
                cs.focus = Some(DocumentPosition::top_level(0, para, byte));
            },
            // Right-click → spelling menu (paginated only). Uses accurate
            // tile-local coordinates from the tile event.
            on_tile_context: move |tile_ctx: TileContext| {
                if view_mode() == ViewMode::Reflow {
                    return;
                }
                open_spell_panel_at(
                    tile_ctx,
                    &doc_state_context,
                    loro_doc,
                    &service,
                    cursor_state,
                    spell_menu,
                );
            },
        }
    }
}
