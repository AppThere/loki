// SPDX-License-Identifier: Apache-2.0

//! Scrollable canvas area for the document editor.
//!
//! [`render_canvas_area`] returns the scroll container div that holds the
//! [`loki_renderer::DocumentView`] component.  It is a plain function
//! (not a Dioxus component) so it cannot call hooks; all reactive state is
//! received as copied signals from [`super::editor_inner::EditorInner`].
//!
//! # Blitz scroll mechanics
//!
//! `WindowEvent::MouseWheel` is handled in blitz-shell without routing through
//! Dioxus (`scroll_node_by_has_changed`, window.rs:388).  For scroll to engage,
//! `can_y_scroll` must be `true` (`overflow-y: auto` or `scroll`) and
//! `scroll_height() > 0`.  Using `flex: 1` left the container height indefinite
//! when Blitz failed to propagate the `height: 100vh` definite size through the
//! flex chain — taffy then expands the container to fit all children, making
//! `scroll_height() = 0`.  The scroll container therefore uses `flex: 1` within
//! a parent that has `height: 100vh` so taffy can resolve the fraction.
//!
//! Scroll position reaches Dioxus through the PATCH(loki) chain: blitz-dom
//! collects the nodes whose offsets changed (`scroll_node_by_collect`),
//! blitz-shell forwards them via `Document::handle_scroll_changes`, and
//! dioxus-native-dom dispatches `scroll` events with `NativeScrollData` —
//! the `onscroll` handler below receives them. The scroll offset is passed to
//! `DocumentView` as `viewport_top_px`, which drives tile virtualization.
//!
//! Click-to-cursor-position is handled by `make_mousedown_handler` in
//! `editor_pointer.rs`, which calls `hit_test_document` and updates the cursor.

use std::sync::Arc;

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_app_shell::spell::SpellService;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::derive_loro_cursor;
use loki_renderer::{DocumentView, RendererCursorPos, TileContext, ViewMode};

use super::editor_canvas_loading::loading_view;
use super::editor_error_view::EditorErrorView;
use super::editor_keydown::make_keydown_handler;
use super::editor_pointer::{make_mousedown_handler, make_mousemove_handler, make_mouseup_handler};
use super::editor_pointer_touch::{
    make_touchend_handler, make_touchmove_handler, make_touchstart_handler,
};
use super::editor_scrollbar::{
    CanvasMounted, ScrollMetrics, ThumbDrag, horizontal_scrollbar, vertical_scrollbar,
};
use super::editor_spell::{SpellMenu, resolve_spell_menu};
use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::editing::hit_test::{link_at_point, open_or_run};
use crate::editing::{hit_test::hit_test_page, state::DocumentState, touch::TouchInteractionState};
use crate::error::LoadError;

/// Fallback viewport height (CSS px) for tile virtualization before the scroll
/// container is first measured. A named default — not a hardcoded screen
/// dimension assumed in a layout path (cf. the 1280px viewport bug, Spec 01
/// audit A-1) — used only for the single frame until `get_client_rect` reports
/// the real height.
const DEFAULT_VIEWPORT_HEIGHT_PX: f64 = 800.0;

/// Right-click handler body: resolves the word under the tile-local coordinates
/// in `ctx` (accurate, via `element_coordinates` — no window-centring math),
/// selects it, and opens the spelling menu anchored at the cursor. A no-op when
/// there is no word at the point.
fn open_spell_panel_at(
    ctx: TileContext,
    doc_state: &Arc<std::sync::Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    service: &SpellService,
    mut cursor_state: Signal<CursorState>,
    mut spell_menu: Signal<Option<SpellMenu>>,
) {
    let layout_opt = {
        let Ok(s) = doc_state.lock() else { return };
        s.paginated_layout.clone()
    };
    let Some(layout) = layout_opt else { return };
    let Some(pos) = hit_test_page(ctx.page_index, ctx.x_pt, ctx.y_pt, &layout) else {
        return;
    };
    match resolve_spell_menu(loro_doc, service, pos.paragraph_index, pos.byte_offset) {
        Some(mut menu) => {
            // Anchor the floating menu at the cursor (window-relative coords).
            menu.anchor_x = ctx.client_x;
            menu.anchor_y = ctx.client_y;
            // Select the whole word so the user sees what the suggestions apply to.
            let word_pos = |byte_offset| {
                DocumentPosition::top_level(pos.page_index, menu.paragraph_index, byte_offset)
            };
            cursor_state.write().anchor = Some(word_pos(menu.byte_start));
            cursor_state.write().focus = Some(word_pos(menu.byte_end));
            spell_menu.set(Some(menu));
        }
        // No word at the point — just place the caret.
        None => {
            cursor_state.write().anchor = Some(pos.clone());
            cursor_state.write().focus = Some(pos);
        }
    }
}

/// Renders the scrollable canvas area for the document editor.
///
/// Plain function — no hooks allowed.  All reactive state is passed in as
/// copied signals.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_canvas_area(
    doc_state_mousedown: std::sync::Arc<std::sync::Mutex<DocumentState>>,
    doc_state_mousemove: std::sync::Arc<std::sync::Mutex<DocumentState>>,
    doc_state_touch: std::sync::Arc<std::sync::Mutex<DocumentState>>,
    doc_state_touchend: std::sync::Arc<std::sync::Mutex<DocumentState>>,
    doc_state_keydown: std::sync::Arc<std::sync::Mutex<DocumentState>>,
    doc_state_render: std::sync::Arc<std::sync::Mutex<DocumentState>>,
    doc_state_scroll: std::sync::Arc<std::sync::Mutex<DocumentState>>,
    is_dragging: Signal<bool>,
    drag_origin: Signal<Option<(f32, f32)>>,
    touch_state: Signal<Option<TouchInteractionState>>,
    mut scroll_offset: Signal<f32>,
    mut scroll_metrics: Signal<ScrollMetrics>,
    mut canvas_mounted: CanvasMounted,
    vbar_drag: ThumbDrag,
    hbar_drag: ThumbDrag,
    mut current_page: Signal<u32>,
    total_pages: Signal<u32>,
    view_mode: Signal<ViewMode>,
    mut cursor_state: Signal<CursorState>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    undo_manager: Signal<Option<loro::UndoManager>>,
    can_undo: Signal<bool>,
    can_redo: Signal<bool>,
    save_request: Signal<u32>,
    path_signal: Signal<String>,
    document_load: Resource<(String, Result<Document, LoadError>)>,
    mut canvas_hovered: Signal<bool>,
    page_gap_px: f32,
    service: SpellService,
    spell_menu: Signal<Option<SpellMenu>>,
    doc_state_context: Arc<std::sync::Mutex<DocumentState>>,
    zoom_percent: Signal<u32>,
    // `macro_run_request`: set to the proc name when a MACROBUTTON (`loki-macro:`
    // link) is clicked, so `editor_macro_notice` dispatches a gated run (§6).
    macro_run_request: Signal<Option<String>>,
) -> Element {
    rsx! {
        // Outer wrapper: the editor column's flex:1 slot — scroll viewport
        // beside custom scrollbar indicators (Blitz paints no scrollbar
        // chrome; see editor_scrollbar).
        div {
            style: "flex: 1; min-height: 0; display: flex; flex-direction: column;",
            div {
                style: "flex: 1; min-height: 0; display: flex; flex-direction: row;",
        div {
            // COMPAT(dioxus-native): flex: 1 needs height: 100vh on the parent
            // for Taffy to resolve the fraction. tabindex="0" + autofocus give the
            // canvas keyboard focus on mount so the user types without clicking.
            //
            // overflow-x: auto (was hidden) lets the user pan a page wider than the
            // viewport (US-Letter on a narrow phone, or while zoomed); the patched
            // Blitz shell synthesises horizontal touch-drag into a scroll here
            // (window.rs), and `can_x_scroll` needs overflow-x auto/scroll.
            //
            // COMPAT(dioxus-native): scrollbar-width / scrollbar-color are Stylo
            // properties blitz-paint 0.2.x does not paint (no scrollbar chrome);
            // kept as forward-compatible hints — scrolling works via touch/wheel.
            //
            // inputmode="text" marks this a text surface so the patched Blitz shell
            // raises the Android soft keyboard on focus (window.rs). Without it the
            // on-screen keyboard never appears on mobile.
            style: format!(
                "flex: 1; min-width: 0; min-height: 0; overflow-y: auto; overflow-x: auto; \
                 background: {bg}; padding: {p}px 0; \
                 scrollbar-width: thin; scrollbar-color: {thumb} transparent;",
                bg    = tokens::COLOR_SURFACE_BASE,
                p     = tokens::SPACE_6,
                thumb = if canvas_hovered() {
                    tokens::COLOR_SCROLLBAR_THUMB_HOVER
                } else {
                    tokens::COLOR_SCROLLBAR_THUMB
                },
            ),
            tabindex: "0",
            autofocus: "true",
            inputmode: "text",

            // Capture the scroll container's MountedData so the scrollbar thumbs
            // can drive programmatic scrolling (dioxus-native scroll_to patch).
            onmounted: move |evt: MountedEvent| {
                canvas_mounted.set(Some(evt));
            },
            onmouseenter: move |_| { canvas_hovered.set(true); },
            onmouseleave: move |_| { canvas_hovered.set(false); },

            // Scroll events are dispatched by the patched Blitz shell
            // (PATCH(loki) in blitz-shell/blitz-dom/dioxus-native-dom) after a
            // wheel or touch gesture changes this container's scroll offset.
            // Updates the status-bar page indicator: the current page is the
            // one occupying the vertical centre of the viewport.
            onscroll: move |evt: ScrollEvent| {
                let top = evt.scroll_top() as f32;
                scroll_offset.set(top);
                let viewport_h = evt.client_height() as f32;
                // Mirror the full geometry so the custom scrollbars can size and
                // place their thumbs.  scroll_width / scroll_height are the
                // scrollable distance (content − client); see editor_scrollbar.
                scroll_metrics.set(ScrollMetrics {
                    scroll_top: top,
                    scroll_left: evt.scroll_left() as f32,
                    scroll_width: evt.scroll_width() as f32,
                    scroll_height: evt.scroll_height() as f32,
                    client_width: evt.client_width() as f32,
                    client_height: viewport_h,
                });
                let (page_h, count) = match doc_state_scroll.lock() {
                    Ok(s) => (s.page_height_px, s.page_count),
                    Err(_) => return,
                };
                // Tiles are painted at `zoom` scale (the inter-page gap is a
                // fixed, unscaled CSS margin), so the page stride the scroll
                // offset measures against is `page_h × zoom + gap`.
                let zoom = zoom_percent() as f32 / 100.0;
                let slot = page_h * zoom + page_gap_px;
                if slot <= 0.0 || count == 0 {
                    return;
                }
                let page = (((top + viewport_h * 0.5) / slot).floor() as i64 + 1)
                    .clamp(1, count as i64) as u32;
                if *current_page.peek() != page {
                    current_page.set(page);
                }
            },

            onmousedown: make_mousedown_handler(drag_origin),

            onmousemove: make_mousemove_handler(
                doc_state_mousemove,
                is_dragging,
                drag_origin,
                scroll_metrics,
                scroll_offset,
                cursor_state,
                page_gap_px,
                view_mode,
                zoom_percent,
            ),

            onmouseup: make_mouseup_handler(is_dragging, drag_origin),

            ontouchstart: make_touchstart_handler(
                std::sync::Arc::clone(&doc_state_touch),
                touch_state,
                cursor_state,
                scroll_offset,
                scroll_metrics,
                view_mode,
                zoom_percent,
                page_gap_px,
            ),

            ontouchmove: make_touchmove_handler(
                doc_state_touch,
                touch_state,
                scroll_offset,
                loro_doc,
                cursor_state,
                page_gap_px,
                view_mode,
                scroll_metrics,
                zoom_percent,
            ),

            ontouchend: make_touchend_handler(
                doc_state_touchend,
                touch_state,
                scroll_offset,
                loro_doc,
                cursor_state,
                page_gap_px,
                view_mode,
                scroll_metrics,
                zoom_percent,
            ),

            onkeydown: make_keydown_handler(
                doc_state_keydown,
                cursor_state,
                loro_doc,
                undo_manager,
                can_undo,
                can_redo,
                save_request,
                view_mode,
                scroll_metrics,
            ),

            match &*document_load.value().read_unchecked() {
                // Gate on `total_pages > 0`: the document has loaded *and* the
                // first paginated layout is ready (published by the deferred
                // Loro-bridge task in editor_inner). Until then the resource may
                // be Ok but `paginated_layout` is still None, which would render
                // a blank canvas — keep the loading indicator up instead.
                Some((loaded_path, Ok(doc)))
                    if loaded_path == &path_signal() && total_pages() > 0 =>
                {
                    // Use the live post-mutation document from doc_state when
                    // available; fall back to the original resource doc before
                    // seed_layout_from_document has run. Read the matching
                    // paginated layout under the same lock so the renderer can
                    // reuse it (single canonical layout) instead of recomputing.
                    let (doc_opt, paginated_layout) = match doc_state_render.lock() {
                        Ok(s) => (s.document.clone(), s.paginated_layout.clone()),
                        Err(_) => (None, None),
                    };
                    let arc_doc = doc_opt.unwrap_or_else(|| Arc::new(doc.clone()));
                    let (cursor_pos, selection_anchor) = {
                        let cs = cursor_state.read();
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
                    rsx! {
                        DocumentView {
                            doc: arc_doc,
                            paginated_layout,
                            zoom: zoom_percent() as f64 / 100.0,
                            // Real measured viewport height (falls back to a
                            // sensible default before the first measure). Drives
                            // tile virtualization: only pages within ~one screen
                            // of the viewport are GPU-rendered.
                            viewport_height_px: {
                                let h = scroll_metrics().client_height as f64;
                                if h > 1.0 { h } else { DEFAULT_VIEWPORT_HEIGHT_PX }
                            },
                            // Real scroll offset so the renderer can virtualize
                            // tiles to the viewport (this scroll container is the
                            // editor's, so the position must be passed in).
                            viewport_top_px: scroll_offset() as f64,
                            cursor_pos,
                            selection_anchor,
                            view_mode: view_mode(),
                            // Width for reflow layout; <= 0 until the canvas is
                            // measured (mount rect or first scroll event).
                            reflow_width_px: scroll_metrics().client_width as f64,
                            // Design tokens injected so the render layer need not
                            // depend on appthere_ui (Spec 01 audit A-8).
                            page_gap_px: tokens::PAGE_GAP_PX as f64,
                            content_padding_bottom_px: tokens::SPACE_6,
                            // Paginated: hit-test against the editor's paginated
                            // layout (reflow clicks arrive via on_reflow_click).
                            on_tile_click: move |c: (usize, f32, f32, bool)| {
                                let (page_index, x_pt, y_pt, open_link) = c;
                                let layout_opt = {
                                    let Ok(state) = doc_state_mousedown.lock() else { return };
                                    state.paginated_layout.clone()
                                };
                                let Some(layout) = layout_opt else { return };
                                if open_link
                                    && let Some(url) = link_at_point(&layout, page_index, x_pt, y_pt)
                                {
                                    open_or_run(&url, macro_run_request);
                                    return; // Ctrl/Cmd+click hit a link/button; no caret move.
                                }
                                let Some(pos) = hit_test_page(page_index, x_pt, y_pt, &layout)
                                else {
                                    return;
                                };
                                let loro_cursor = loro_doc.read().as_ref().and_then(|ldoc| {
                                    derive_loro_cursor(ldoc, pos.paragraph_index, pos.byte_offset)
                                });
                                let mut cs = cursor_state.write();
                                cs.loro_cursor = loro_cursor;
                                cs.anchor = Some(pos.clone());
                                cs.focus = Some(pos);
                            },
                            // Reflow: DocumentView already resolved the click to a
                            // (paragraph, byte) position in the continuous layout.
                            on_reflow_click: move |(para, byte): (usize, usize)| {
                                let loro_cursor = loro_doc.read().as_ref().and_then(|ldoc| {
                                    derive_loro_cursor(ldoc, para, byte)
                                });
                                // page_index is meaningless in reflow (the caret is
                                // painted from paragraph/byte); 0 is a placeholder.
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
                            // Reflow drag-select: move only the focus, keeping the
                            // anchor so a range selection grows under the pointer.
                            on_reflow_drag: move |(para, byte): (usize, usize)| {
                                let loro_cursor = loro_doc.read().as_ref().and_then(|ldoc| {
                                    derive_loro_cursor(ldoc, para, byte)
                                });
                                let mut cs = cursor_state.write();
                                cs.loro_cursor = loro_cursor;
                                cs.focus = Some(DocumentPosition::top_level(0, para, byte));
                            },
                            // Right-click → spelling menu (paginated only). Uses
                            // accurate tile-local coordinates from the tile event.
                            on_tile_context: move |ctx: TileContext| {
                                if view_mode() == ViewMode::Reflow {
                                    return;
                                }
                                open_spell_panel_at(
                                    ctx,
                                    &doc_state_context,
                                    loro_doc,
                                    &service,
                                    cursor_state,
                                    spell_menu,
                                );
                            },
                        }
                    }
                },

                Some((loaded_path, Err(e))) if loaded_path == &path_signal() => {
                    let msg = e.to_string();
                    rsx! { EditorErrorView { message: msg } }
                },

                // Resource still pending (file being read / imported), or the
                // resolved value is for a previous path during a tab switch:
                // show the blank page placeholder with a loading label.
                _ => loading_view(),
            }
        }
                // Vertical scroll indicator + drag handle (right-edge gutter).
                {vertical_scrollbar(
                    scroll_metrics(),
                    current_page(),
                    total_pages(),
                    canvas_mounted,
                    vbar_drag,
                )}
            }
            // Horizontal scroll indicator + drag handle (bottom; only when the
            // page is wider than the viewport).
            {horizontal_scrollbar(scroll_metrics(), canvas_mounted, hbar_drag)}
        }
    }
}
