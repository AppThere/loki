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
use loki_doc_model::document::Document;
use loki_renderer::{DocumentView, RendererCursorPos, ViewMode};

use super::editor_error_view::EditorErrorView;
use super::editor_keydown::make_keydown_handler;
use super::editor_pointer::{
    make_mousemove_handler, make_touchend_handler, make_touchmove_handler,
};
use super::editor_scrollbar::{
    CanvasMounted, ScrollMetrics, ThumbDrag, horizontal_scrollbar, vertical_scrollbar,
};
use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::editing::hit_test::{hit_test_document, hit_test_page};
use crate::editing::state::DocumentState;
use crate::editing::touch::TouchInteractionState;
use loki_app_shell::spell::SpellService;

use super::editor_spell::{SpellMenu, resolve_spell_menu};
use crate::error::LoadError;
use loki_doc_model::loro_bridge::derive_loro_cursor;
use loki_i18n::fl;

/// Blank page placeholder shown while a document is being opened.
///
/// Renders immediately when the editor tab mounts (before the async load
/// resolves), so the user sees a page-shaped surface with an "opening" label
/// instead of an empty canvas while the file is read, imported, and laid out.
fn loading_view() -> Element {
    rsx! {
        div {
            style: format!(
                "display: flex; flex: 1; align-items: flex-start; \
                 justify-content: center; width: 100%; padding-top: {gap}px;",
                gap = tokens::SPACE_6,
            ),
            div {
                style: format!(
                    "width: {w}px; height: {h}px; flex-shrink: 0; background: {page}; \
                     border: 1px solid {border}; border-radius: 2px; display: flex; \
                     align-items: center; justify-content: center;",
                    w = tokens::PAGE_WIDTH_PX,
                    h = tokens::PAGE_HEIGHT_PX,
                    page = tokens::CANVAS_PAGE_BG,
                    border = tokens::COLOR_BORDER_CHROME,
                ),
                span {
                    style: format!(
                        "font-family: {ff}; font-size: {fs}px; color: {fg};",
                        ff = tokens::FONT_FAMILY_UI,
                        fs = tokens::FONT_SIZE_BODY,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    { fl!("editor-document-loading") }
                }
            }
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
    mut is_dragging: Signal<bool>,
    mut drag_origin: Signal<Option<(f32, f32)>>,
    touch_state: Signal<Option<TouchInteractionState>>,
    window_width: Signal<f32>,
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
    mut spell_menu: Signal<Option<SpellMenu>>,
    doc_state_context: Arc<std::sync::Mutex<DocumentState>>,
) -> Element {
    rsx! {
        // Outer wrapper occupies the editor column's flex:1 slot and lays out
        // the scroll viewport beside a vertical scrollbar, with a horizontal
        // scrollbar underneath.  Blitz paints no scrollbar chrome, so these are
        // custom indicators (see editor_scrollbar).
        div {
            style: "flex: 1; min-height: 0; display: flex; flex-direction: column;",
            div {
                style: "flex: 1; min-height: 0; display: flex; flex-direction: row;",
        div {
            // COMPAT(dioxus-native): flex: 1 is confirmed working. Requires
            // height: 100vh on the parent so Taffy can resolve the flex fraction.
            // tabindex="0" enables keyboard focus for onkeydown to fire.
            // autofocus ensures the canvas receives keyboard focus immediately
            // when the editor mounts, so the user can type without clicking first.
            //
            // overflow-x: auto (was hidden) lets the user pan a page that is
            // wider than the viewport — e.g. a US-Letter page on a narrow phone,
            // or any page while zoomed in.  The patched Blitz shell synthesises
            // horizontal touch-drag into a scroll on this container (window.rs),
            // and `can_x_scroll` is only true when overflow-x is auto/scroll.
            //
            // COMPAT(dioxus-native): scrollbar-width / scrollbar-color are Stylo
            // (Firefox CSS engine) properties that blitz-paint 0.2.x does not
            // paint — Blitz renders no scrollbar chrome at all.  They are kept
            // as forward-compatible hints; scrolling itself works via touch/wheel
            // regardless.  A visible scrollbar requires a custom widget.
            //
            // inputmode="text" marks this as a text-editing surface so the
            // patched Blitz shell raises the Android soft keyboard when the
            // canvas gains focus (window.rs::update_ime_for_focus).  Without it
            // the on-screen keyboard never appears on mobile.
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
                let slot = page_h + page_gap_px;
                if slot <= 0.0 || count == 0 {
                    return;
                }
                let page = (((top + viewport_h * 0.5) / slot).floor() as i64 + 1)
                    .clamp(1, count as i64) as u32;
                if *current_page.peek() != page {
                    current_page.set(page);
                }
            },

            // Outer div records drag origin; cursor placement happens in
            // on_tile_click on the per-page div (element_coordinates, no origin math).
            onmousedown: move |evt: MouseEvent| {
                let c = evt.client_coordinates();
                drag_origin.set(Some((c.x as f32, c.y as f32)));
            },

            // Right-click: open the spelling suggestions panel for the word under
            // the pointer (paginated mode). Resolves the click to a document
            // position via the same hit-test as drag-select, then builds a
            // SpellMenu. COMPAT(dioxus-native): relies on oncontextmenu +
            // prevent_default being honoured by the Blitz shell.
            oncontextmenu: move |evt: MouseEvent| {
                evt.prevent_default();
                if view_mode() == ViewMode::Reflow {
                    return;
                }
                let c = evt.client_coordinates();
                let (layout_opt, pw, ph) = {
                    let Ok(s) = doc_state_context.lock() else { return };
                    (s.paginated_layout.clone(), s.page_width_px, s.page_height_px)
                };
                let Some(layout) = layout_opt else { return };
                let x_off = (window_width() - pw).max(0.0) / 2.0;
                let origin = (x_off, tokens::TOOLBAR_HEIGHT_TOP + tokens::SPACE_6);
                if let Some(pos) = hit_test_document(
                    c.x as f32, c.y as f32, origin, scroll_offset(), &layout, pw, ph, page_gap_px,
                ) {
                    cursor_state.write().focus = Some(pos.clone());
                    cursor_state.write().anchor = Some(pos.clone());
                    if let Some(menu) =
                        resolve_spell_menu(loro_doc, &service, pos.paragraph_index, pos.byte_offset)
                    {
                        spell_menu.set(Some(menu));
                    }
                }
            },

            onmousemove: make_mousemove_handler(
                doc_state_mousemove,
                is_dragging,
                drag_origin,
                window_width,
                scroll_offset,
                cursor_state,
                page_gap_px,
                view_mode,
            ),

            onmouseup: move |_| {
                is_dragging.set(false);
                drag_origin.set(None);
            },

            ontouchstart: move |evt: TouchEvent| {
                let touches = evt.touches();
                let Some(first) = touches.first() else { return };
                let c = first.client_coordinates();
                touch_state.clone().set(Some(TouchInteractionState::new(
                    0,
                    (c.x as f32, c.y as f32),
                )));
            },

            ontouchmove: make_touchmove_handler(
                doc_state_touch,
                touch_state,
                window_width,
                scroll_offset,
                loro_doc,
                cursor_state,
                page_gap_px,
                view_mode,
                scroll_metrics,
            ),

            ontouchend: make_touchend_handler(
                doc_state_touchend,
                touch_state,
                window_width,
                scroll_offset,
                loro_doc,
                cursor_state,
                page_gap_px,
                view_mode,
                scroll_metrics,
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
                            // Real measured viewport height (falls back to a
                            // sensible default before the first measure). Drives
                            // tile virtualization: only pages within ~one screen
                            // of the viewport are GPU-rendered.
                            viewport_height_px: {
                                let h = scroll_metrics().client_height as f64;
                                if h > 1.0 { h } else { 800.0 }
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
                            // Paginated: hit-test against the editor's paginated
                            // layout (reflow clicks are hit-tested inside
                            // DocumentView and arrive via on_reflow_click).
                            on_tile_click: move |(page_index, x_pt, y_pt): (usize, f32, f32)| {
                                let layout_opt = {
                                    let Ok(state) = doc_state_mousedown.lock() else { return };
                                    state.paginated_layout.clone()
                                };
                                let Some(layout) = layout_opt else { return };
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
                                let pos = DocumentPosition {
                                    // page_index is meaningless in reflow; 0 is a
                                    // safe placeholder (the caret is painted from
                                    // paragraph/byte, not page, in reflow mode).
                                    page_index: 0,
                                    paragraph_index: para,
                                    byte_offset: byte,
                                };
                                let mut cs = cursor_state.write();
                                cs.loro_cursor = loro_cursor;
                                cs.anchor = Some(pos.clone());
                                cs.focus = Some(pos);
                            },
                            // Reflow drag-select: move only the focus, keeping the
                            // anchor so a range selection grows under the pointer.
                            on_reflow_drag: move |(para, byte): (usize, usize)| {
                                let loro_cursor = loro_doc.read().as_ref().and_then(|ldoc| {
                                    derive_loro_cursor(ldoc, para, byte)
                                });
                                let mut cs = cursor_state.write();
                                cs.loro_cursor = loro_cursor;
                                cs.focus = Some(DocumentPosition {
                                    page_index: 0,
                                    paragraph_index: para,
                                    byte_offset: byte,
                                });
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
