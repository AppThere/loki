// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Editor screen route component.
//!
//! Implements the document editor shell.  The layout is a vertical flex column:
//!
//! ```text
//! ┌─────────────────────────┐
//! │      Top Toolbar        │  flex-shrink: 0
//! ├─────────────────────────┤
//! │                         │
//! │  ┌───────────────────┐  │  height: calc(100vh - chrome), overflow-y: auto
//! │  │   Page 1          │  │
//! │  └───────────────────┘  │
//! │  ┌───────────────────┐  │
//! │  │   Page 2          │  │
//! │  └───────────────────┘  │
//! │         …               │
//! │                         │
//! ├─────────────────────────┤
//! │     Bottom Toolbar      │  flex-shrink: 0
//! └─────────────────────────┘
//! ```
//!
//! The `path` route parameter carries a serialised
//! [`loki_file_access::FileAccessToken`].  Document loading runs in a
//! [`use_resource`] async task so the shell renders immediately while the
//! import pipeline runs in the background.

// Pipeline entry points (confirmed from source):
// loki_file_access: FileAccessToken::deserialize(s: &str) -> Result<FileAccessToken, TokenParseError>
//                   token.open_read() -> Result<Box<dyn ReadSeek>, AccessError>
//                   where ReadSeek: std::io::Read + std::io::Seek + Send
// loki_ooxml:       DocxImport::import(reader: impl Read + Seek, options: DocxImportOptions)
//                       -> Result<loki_doc_model::Document, OoxmlError>
//                   (via loki_doc_model::io::DocumentImport trait)
// loki_vello:       paint_layout(scene: &mut vello::Scene, layout: &DocumentLayout,
//                       font_cache: &mut FontDataCache, offset: (f32, f32), scale: f32,
//                       page_index: Option<usize>)
//                   (called inside WgpuSurface — see components/wgpu_surface.rs)

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use appthere_ui::{AtDocumentTabData, AtStatusBar, AtTabBar};
use dioxus::prelude::*;
use keyboard_types::Modifiers;
use loki_doc_model::document::Document;
use loki_doc_model::io::DocumentImport;
use loki_doc_model::loro_bridge::{derive_loro_cursor, document_to_loro};
use loki_doc_model::loro_mutation::{delete_text, get_block_text, insert_text};
use loki_doc_model::{merge_block, split_block};
use loki_file_access::FileAccessToken;
use loki_layout::LayoutOptions;
use loki_odf::odt::import::{OdtImport, OdtImportOptions};
use loki_ooxml::docx::import::{DocxImport, DocxImportOptions};

use crate::components::document_source::{DocumentState, apply_mutation_and_relayout};
use crate::components::toolbar::TopToolbar;
use crate::components::wgpu_surface::WgpuSurface;
use crate::editing::cursor::{
    CursorState, DocumentPosition, next_grapheme_boundary, prev_grapheme_boundary,
};
use crate::editing::hit_test::hit_test_document;
use crate::editing::navigation::{
    navigate_down, navigate_end, navigate_home, navigate_left, navigate_right, navigate_up,
};
use crate::editing::touch::{TouchInteractionState, TouchPhase, word_boundaries_at};
use crate::error::LoadError;
use crate::tabs::OpenTab;
use crate::utils::display_title_from_path;

/// Editor view mode toggle.
#[derive(Clone, PartialEq, Copy)]
pub enum EditorMode {
    Reading,
    Editing,
}

/// Document editor shell component.
///
/// Receives the `path` route parameter (a serialised
/// [`loki_file_access::FileAccessToken`]) and renders the three-panel editor
/// layout: top toolbar, scrollable page canvas area, and bottom status bar.
///
/// Document loading runs asynchronously via [`use_resource`]:
/// - **Loading** — toolbars are shown immediately; canvas shows "Opening
///   document…" placeholder via [`WgpuSurface`].
/// - **Error** — inline error message with a "Go back" button; no panic.
/// - **Loaded** — document is passed to [`WgpuSurface`] for scene building.
#[component]
pub fn Editor(path: String) -> Element {
    let title = display_title_from_path(&path);

    let editor_mode = use_signal(|| EditorMode::Reading);
    let mut loro_doc: Signal<Option<loro::LoroDoc>> = use_signal(|| None);

    // ── Shared document state ─────────────────────────────────────────────────
    //
    // Created here (not inside WgpuSurface) so that mouse and keyboard handlers
    // can close over the Arc and read `paginated_layout` for hit-testing, or
    // bump `generation` after a Loro mutation to trigger a GPU re-render.
    let doc_state: Arc<Mutex<DocumentState>> = use_hook(|| {
        Arc::new(Mutex::new(DocumentState {
            document: None,
            generation: 0,
            page_count: 0,
            canvas_width: 0.0,
            visible_rect: None,
            page_width_px: tokens::PAGE_WIDTH_PX,
            page_height_px: tokens::PAGE_HEIGHT_PX,
            cursor_state: None,
            paginated_layout: None,
            preserve_for_editing: false,
            shared_renderer: Arc::new(Mutex::new(None)),
            shared_font_cache: Arc::new(Mutex::new(loki_vello::FontDataCache::new())),
            layout_stamp: 0,
            layout_generation: 0,
            layout_canvas_width: 0.0,
            layout_preserve_for_editing: false,
            shared_font_resources: Arc::new(Mutex::new(loki_layout::FontResources::new())),
        }))
    });

    // Pre-clone the Arc once so closures below can each capture their own clone.
    let doc_state_mousemove = Arc::clone(&doc_state);
    let doc_state_touch = Arc::clone(&doc_state);
    let doc_state_prop = Arc::clone(&doc_state);

    // ── Cursor / selection state ──────────────────────────────────────────────
    let mut cursor_state: Signal<CursorState> = use_signal(CursorState::new);
    // `is_dragging`: mouse button is currently held down.
    let mut is_dragging: Signal<bool> = use_signal(|| false);
    // Client-coordinate position of the most recent mousedown.  Used to gate
    // focus updates in `onmousemove` behind a drag threshold so that tiny
    // cursor jitter during a click does not create a spurious text selection.
    let mut drag_origin: Signal<Option<(f32, f32)>> = use_signal(|| None);

    // Touch interaction state — None when no finger is currently down.
    let mut touch_state: Signal<Option<TouchInteractionState>> = use_signal(|| None);

    // window_width and scroll_offset are used only by the onmousemove drag
    // selection handler via hit_test_document.  Click placement (onmousedown)
    // now uses element_coordinates() from the patched dioxus-native-dom and
    // no longer needs these values.
    //
    // window_width: window inner width in CSS px for centering the hit-test
    // origin.  Still defaults to 1280 because Blitz does not expose a
    // window-resize hook to Dioxus components.
    // TODO(window-size): update once Blitz exposes inner_size.
    //
    // scroll_offset: always 0.0 because Blitz does not route scroll events
    // through Dioxus; drag selection will be imprecise after scrolling.
    // TODO(partial-render): wire when Blitz exposes node.scroll_offset.
    let window_width: Signal<f32> = use_signal(|| 1280.0_f32);
    let scroll_offset: Signal<f32> = use_signal(|| 0.0_f32);

    // Kick off the document-loading pipeline.  The future is async but all
    // I/O is synchronous under the hood; a spawn_blocking wrapper would be
    // appropriate for large files once the executor supports it.
    let document_load: Resource<Result<Document, LoadError>> = {
        let path = path.clone();
        use_resource(move || {
            let path = path.clone();
            async move { load_document(path) }
        })
    };

    let navigator = use_navigator();

    use_effect(move || {
        if let Some(Ok(doc)) = &*document_load.value().read_unchecked()
            && loro_doc().is_none()
        {
            match document_to_loro(doc) {
                Ok(l_doc) => loro_doc.set(Some(l_doc)),
                Err(e) => tracing::warn!("Failed to initialize Loro sync bridge: {}", e),
            }
        }
    });

    let layout_opts = match editor_mode() {
        EditorMode::Reading => LayoutOptions::default(),
        EditorMode::Editing => LayoutOptions {
            preserve_for_editing: true,
        },
    };

    let page_gap_px = tokens::PAGE_GAP_PX;

    // ── Scroll container height ───────────────────────────────────────────────
    //
    // Blitz scroll event path (blitz-shell-0.2.3/src/window.rs:388):
    //   WindowEvent::MouseWheel
    //     → scroll_node_by_has_changed(hover_node_id)   [document.rs:1258]
    //     → bubbles DOM tree until a node with can_y_scroll=true is found
    //     → updates node.scroll_offset when scroll_height() > 0
    //     → blitz-paint applies offset as a translate    [render.rs:235-245]
    //
    // Scrollability (blitz-dom-0.2.4/src/document.rs:1272-1277):
    //   can_y_scroll = overflow_y ∈ {Scroll, Auto}
    //   Both values map to taffy::Overflow::Scroll in
    //   stylo_taffy-0.2.0/src/convert.rs:227-228.
    //
    // scroll_height() = max(0, content_size.height − size.height)
    //   (taffy-0.9.2/src/tree/layout.rs:339-344)
    //   For scroll to engage, content (pages) must exceed the container's
    //   computed height.
    //
    // ROOT CAUSE of prior non-functional scroll:
    //   Using `flex: 1` left the scroll container's taffy height indefinite
    //   when Blitz failed to propagate the `height: 100vh` definite size from
    //   the root div through the flex chain.  With an indefinite height taffy
    //   expands the container to fit all children → content_size == size →
    //   scroll_height() == 0 → scroll_node_by_has_changed returns false.
    //
    // FIX: explicit `height: calc(100vh - Npx)` gives taffy a concrete
    //   Dimension::Length(n), bypassing the flex chain entirely.  The pages
    //   overflow the finite box → scroll_height > 0 → scrolling works.
    //
    // KNOWN LIMITATION: no public API in dioxus-native-0.7.4 exposes
    //   node.scroll_offset back to Dioxus components, so visible_rect stays
    //   None.  onwheel handlers never fire; scroll is driven entirely by
    //   blitz-shell's WindowEvent::MouseWheel handler.
    //   TODO(partial-render): wire scroll_offset → visible_rect once Blitz
    //   exposes a scroll-position hook to Dioxus components.
    //
    // ── Toolbar hit-test overlap when scrolled ────────────────────────────────
    //
    // BUG (Blitz): Node::hit() [blitz-dom-0.2.4/src/node/node.rs:716-773]
    //   adjusts incoming coordinates by +scroll_offset before checking bounds:
    //     adjusted_y = pointer_y - container_y + scroll_offset_y
    //     matches_self = !(adjusted_y < 0 || …)
    //   When scroll_offset_y ≥ container_y (= TOOLBAR_HEIGHT_TOP), adjusted_y
    //   is always ≥ 0 for any pointer_y ≥ 0, so the scroll container claims
    //   every click including those in the toolbar row.
    //
    //   pointer-events:none is NOT implemented in this Blitz version
    //   (blitz-dom-0.2.4/src/node/node.rs has no reference to PointerEvents).
    //
    // FIX: paint_children are sorted ascending by z_index
    //   [blitz-dom-0.2.4/src/layout/damage.rs:353-383].  Hit testing iterates
    //   paint_children in REVERSE, so the highest z_index wins.  TopToolbar
    //   carries `position: relative; z-index: 10` (see toolbar.rs); the scroll
    //   container has the default z_index: 0.  After sorting, TopToolbar is the
    //   last entry in paint_children and therefore the FIRST tested — it
    //   captures clicks in its own (correct) bounds before the scroll container
    //   is tried.
    // Tab state — shared via Dioxus context from App root.
    let mut tabs = use_context::<Signal<Vec<OpenTab>>>();
    let mut active_tab = use_context::<Signal<usize>>();

    rsx! {
        div {
            // COMPAT(dioxus-native): height: 100vh gives Taffy a concrete
            // Dimension::Length so flex: 1 on the scroll child resolves to a
            // definite height, enabling overflow-y: auto scroll. Without an
            // explicit height here Blitz cannot propagate the 100vh definite
            // size from the App root through the Router flex chain.
            style: format!(
                "display: flex; flex-direction: column; height: 100vh; \
                 overflow: hidden; background: {bg}; font-family: system-ui, sans-serif;",
                bg = tokens::COLOR_SURFACE_BASE,
            ),

            // TODO(platform): AtTitleBar is omitted on desktop — the OS provides native
            // window chrome. Render AtTitleBar only on Android/iOS once platform
            // detection is wired (see Platform enum in appthere_ui).

            // ── Document tab bar ──────────────────────────────────────────────
            AtTabBar {
                tabs: tabs.read().iter().map(|t| AtDocumentTabData {
                    title:        t.title.clone(),
                    is_dirty:     t.is_dirty,
                    is_discarded: t.is_discarded,
                }).collect(),
                active_index:       *active_tab.read(),
                home_tab_label:     "Home",
                aria_label:         "Open documents",
                new_tab_aria_label: "New document",
                on_tab_select: move |idx| {
                    *active_tab.write() = idx;
                    if idx == 0 {
                        navigator.push(crate::routes::Route::Home {});
                    }
                    // TODO(tabs): Navigate to the correct editor route for idx > 0.
                },
                on_tab_close: move |idx| {
                    if idx > 0 {
                        tabs.write().remove(idx - 1);
                        let new_len = tabs.read().len();
                        let current = *active_tab.read();
                        if current >= idx && current > 0 {
                            *active_tab.write() = current.saturating_sub(1);
                        }
                        if new_len == 0 {
                            navigator.push(crate::routes::Route::Home {});
                        }
                    }
                },
                on_new_tab: move |_| {
                    // TODO(tabs): Open a blank document as a new tab.
                    navigator.push(crate::routes::Route::Home {});
                },
            }

            // ── Top toolbar (flex-shrink: 0) ───────────────────────────────────
            TopToolbar {
                title: title,
                editor_mode: editor_mode
            }

            // ── Scroll container ──────────────────────────────────────────────
            //
            // COMPAT(dioxus-native): flex: 1 is confirmed working. Removing
            // calc() height in favour of flex growth to accommodate dynamic
            // chrome heights. Requires height: 100vh on the parent editor div
            // so Taffy can resolve the flex fraction to a concrete length.
            div {
                style: format!(
                    "flex: 1; min-height: 0; overflow-y: auto; overflow-x: hidden; \
                     background: {bg}; padding: {p}px 0;",
                    bg = tokens::COLOR_SURFACE_BASE,
                    p  = tokens::SPACE_6,
                ),

                // ── Pointer event handlers for cursor / selection ──────────────
                //
                // All three handlers guard on EditorMode::Editing so no cursor
                // state is modified in read-only mode.  client_coordinates()
                // provides window-relative CSS logical pixels.
                //
                // onmousedown on the scroll container fires via bubbling from
                // child canvas elements.  It records the raw client position so
                // that onmousemove can apply a drag threshold before extending
                // the selection — preventing cursor jitter during a plain click
                // from creating a spurious text selection.
                onmousedown: move |evt| {
                    let c = evt.client_coordinates();
                    drag_origin.set(Some((c.x as f32, c.y as f32)));
                },

                onmousemove: {
                    let doc_state = Arc::clone(&doc_state_mousemove);
                    move |evt| {
                        if !is_dragging() || editor_mode() != EditorMode::Editing {
                            return;
                        }

                        // Only extend the selection once the pointer has moved
                        // beyond DRAG_THRESHOLD_PX from the mousedown origin.
                        // This prevents tiny cursor jitter during a click from
                        // creating a spurious selection via hit_test_document.
                        const DRAG_THRESHOLD_SQ: f32 = 4.0 * 4.0; // 4 CSS px
                        let coords = evt.client_coordinates();
                        let cx = coords.x as f32;
                        let cy = coords.y as f32;
                        if let Some((ox, oy)) = drag_origin() {
                            let dx = cx - ox;
                            let dy = cy - oy;
                            if dx * dx + dy * dy < DRAG_THRESHOLD_SQ {
                                return;
                            }
                        }

                        // Read layout and page dimensions from shared state.
                        let (layout_opt, page_width_px, page_height_px) = {
                            let Ok(state) = doc_state.lock() else { return; };
                            (
                                state.paginated_layout.clone(),
                                state.page_width_px,
                                state.page_height_px,
                            )
                        };

                        let Some(layout) = layout_opt else { return; };

                        let x_off = (window_width() - page_width_px).max(0.0) / 2.0;
                        let origin = (x_off, tokens::TOOLBAR_HEIGHT_TOP + tokens::SPACE_6);

                        let pos = hit_test_document(
                            cx,
                            cy,
                            origin,
                            scroll_offset(),
                            &layout,
                            page_width_px,
                            page_height_px,
                            page_gap_px,
                        );

                        // During drag: update focus only, anchor stays fixed.
                        if let Some(p) = pos {
                            cursor_state.write().focus = Some(p);
                        }
                    }
                },

                onmouseup: move |_| {
                    is_dragging.set(false);
                    drag_origin.set(None);
                },

                // ── Touch event handlers ───────────────────────────────────────
                //
                // blitz-shell synthesises touch contacts as mouse events so
                // ontouchstart / ontouchmove / ontouchend fire via the normal
                // Dioxus event pipeline.  The handlers here run on top of those
                // synthesised events and implement the loki-text touch UX
                // (tap → cursor, drag → scroll, long-press → word selection).
                // They guard on EditorMode::Editing so read mode is unaffected.
                ontouchstart: {
                    move |evt: TouchEvent| {
                        if editor_mode() != EditorMode::Editing {
                            return;
                        }
                        let touches = evt.touches();
                        let Some(first) = touches.first() else { return; };
                        let c = first.client_coordinates();
                        let pos = (c.x as f32, c.y as f32);
                        touch_state.set(Some(TouchInteractionState::new(0, pos)));
                    }
                },

                ontouchmove: {
                    let doc_state = Arc::clone(&doc_state_touch);
                    move |evt: TouchEvent| {
                        if editor_mode() != EditorMode::Editing {
                            return;
                        }
                        let Some(mut ts) = touch_state() else { return; };
                        let touches = evt.touches();
                        let Some(first) = touches.first() else { return; };
                        let c = first.client_coordinates();
                        let new_pos = (c.x as f32, c.y as f32);

                        let became_scroll = ts.update_move(new_pos);

                        if became_scroll {
                            if let TouchPhase::Scroll { last_y } = ts.phase {
                                // The scroll container is driven by blitz-shell's
                                // native scroll mechanism; we update scroll_offset
                                // here so hit_test_document stays accurate.
                                // TODO(partial-render): replace with direct
                                // node.scroll_offset once Blitz exposes it.
                                let _ = last_y; // used in future scroll integration
                            }
                        } else if ts.phase == TouchPhase::LongPress {
                            // Long-press detected — trigger word selection at
                            // the original touch position.
                            let start = ts.start_pos;
                            let (layout_opt, page_width_px, page_height_px) = {
                                let Ok(state) = doc_state.lock() else { return; };
                                (
                                    state.paginated_layout.clone(),
                                    state.page_width_px,
                                    state.page_height_px,
                                )
                            };
                            if let Some(layout) = layout_opt {
                                let x_off = (window_width() - page_width_px).max(0.0) / 2.0;
                                let origin =
                                    (x_off, tokens::TOOLBAR_HEIGHT_TOP + tokens::SPACE_6);
                                if let Some(pos) = hit_test_document(
                                    start.0,
                                    start.1,
                                    origin,
                                    scroll_offset(),
                                    &layout,
                                    page_width_px,
                                    page_height_px,
                                    page_gap_px,
                                ) {
                                    let ldoc_guard = loro_doc.read();
                                    if let Some(ldoc) = ldoc_guard.as_ref() {
                                        let text =
                                            loki_doc_model::loro_mutation::get_block_text(
                                                ldoc,
                                                pos.paragraph_index,
                                            );
                                        if let Some((ws, we)) =
                                            word_boundaries_at(&text, pos.byte_offset)
                                        {
                                            let anchor = DocumentPosition {
                                                page_index: pos.page_index,
                                                paragraph_index: pos.paragraph_index,
                                                byte_offset: ws,
                                            };
                                            let focus = DocumentPosition {
                                                page_index: pos.page_index,
                                                paragraph_index: pos.paragraph_index,
                                                byte_offset: we,
                                            };
                                            let mut cs = cursor_state.write();
                                            cs.anchor = Some(anchor);
                                            cs.focus = Some(focus);
                                        }
                                    }
                                }
                            }
                        }

                        touch_state.set(Some(ts));
                    }
                },

                ontouchend: {
                    let doc_state = Arc::clone(&doc_state);
                    move |_evt: TouchEvent| {
                        if editor_mode() != EditorMode::Editing {
                            touch_state.set(None);
                            return;
                        }
                        let Some(ts) = touch_state() else { return; };

                        match ts.phase {
                            TouchPhase::Indeterminate | TouchPhase::Tap => {
                                // Short tap — place cursor via the same hit-test
                                // path as a mouse click.
                                let (layout_opt, page_width_px, page_height_px) = {
                                    let Ok(state) = doc_state.lock() else {
                                        touch_state.set(None);
                                        return;
                                    };
                                    (
                                        state.paginated_layout.clone(),
                                        state.page_width_px,
                                        state.page_height_px,
                                    )
                                };
                                if let Some(layout) = layout_opt {
                                    let x_off =
                                        (window_width() - page_width_px).max(0.0) / 2.0;
                                    let origin = (
                                        x_off,
                                        tokens::TOOLBAR_HEIGHT_TOP + tokens::SPACE_6,
                                    );
                                    if let Some(pos) = hit_test_document(
                                        ts.start_pos.0,
                                        ts.start_pos.1,
                                        origin,
                                        scroll_offset(),
                                        &layout,
                                        page_width_px,
                                        page_height_px,
                                        page_gap_px,
                                    ) {
                                        let loro_cursor =
                                            loro_doc.read().as_ref().and_then(|ldoc| {
                                                loki_doc_model::loro_bridge::derive_loro_cursor(
                                                    ldoc,
                                                    pos.paragraph_index,
                                                    pos.byte_offset,
                                                )
                                            });
                                        let mut cs = cursor_state.write();
                                        cs.loro_cursor = loro_cursor;
                                        cs.anchor = Some(pos.clone());
                                        cs.focus = Some(pos);
                                    }
                                }
                            }
                            // Scroll and long-press states are already handled
                            // incrementally in ontouchmove.
                            TouchPhase::Scroll { .. } | TouchPhase::LongPress => {}
                        }

                        touch_state.set(None);
                    }
                },

                // ── Keyboard input (forwarded from WgpuSurface) ───────────────
                //
                // onkeydown lives on the WgpuSurface outer div (tabindex="0")
                // so keyboard focus follows mouse clicks on the page canvas.
                // WgpuSurface forwards events via its on_keydown prop.
                // The handler is defined at the WgpuSurface call sites below.

                match &*document_load.value().read_unchecked() {
                    // Resource is still running — show placeholder via WgpuSurface.
                    None => rsx! {
                        WgpuSurface {
                            doc_state: Arc::clone(&doc_state_prop),
                            document: None,
                            layout_opts: layout_opts.clone(),
                            visible_rect: None,
                            cursor_state: None,
                            on_mousedown: |_| {},
                            on_keydown: |_| {},
                        }
                    },

                    // Import pipeline failed.
                    Some(Err(e)) => {
                        let msg = e.to_string();
                        rsx! {
                            div {
                                style: format!(
                                    "display: flex; flex-direction: column; \
                                     justify-content: center; align-items: center; \
                                     gap: {gap}px;",
                                    gap = tokens::SPACE_4,
                                ),
                                span {
                                    style: format!(
                                        "font-size: {size}px; color: {fg};",
                                        size = tokens::FONT_SIZE_BODY,
                                        fg   = tokens::COLOR_TEXT_PRIMARY,
                                    ),
                                    "Could not open document: {msg}"
                                }
                                button {
                                    style: format!(
                                        "padding: {p}px {p2}px; background: {bg}; \
                                         border: 1px solid {border}; border-radius: 4px; \
                                         font-size: {size}px; cursor: pointer;",
                                        p      = tokens::SPACE_2,
                                        p2     = tokens::SPACE_4,
                                        bg     = tokens::COLOR_SURFACE_PAGE,
                                        border = tokens::COLOR_BORDER_DEFAULT,
                                        size   = tokens::FONT_SIZE_BODY,
                                    ),
                                    onclick: move |_| { navigator.push(crate::routes::Route::Home {}); },
                                    "Go back"
                                }
                            }
                        }
                    },

                    Some(Ok(doc)) => {
                        let cs = if editor_mode() == EditorMode::Editing {
                            Some(cursor_state.read().clone())
                        } else {
                            None
                        };
                        rsx! {
                            WgpuSurface {
                                doc_state: Arc::clone(&doc_state_prop),
                                document: Some(doc.clone()),
                                layout_opts: layout_opts.clone(),
                                visible_rect: None,
                                cursor_state: cs,
                                on_mousedown: move |p: DocumentPosition| {
                                    if editor_mode() != EditorMode::Editing {
                                        return;
                                    }
                                    is_dragging.set(true);

                                    let loro_cursor = loro_doc.read().as_ref().and_then(|ldoc| {
                                        derive_loro_cursor(ldoc, p.paragraph_index, p.byte_offset)
                                    });
                                    let mut cs = cursor_state.write();
                                    cs.loro_cursor = loro_cursor;
                                    cs.anchor = Some(p.clone());
                                    cs.focus = Some(p);
                                },
                                on_keydown: {
                                    let doc_state = Arc::clone(&doc_state);
                                    move |evt: Rc<KeyboardData>| {
                                        if editor_mode() != EditorMode::Editing {
                                            return;
                                        }

                                        let key = evt.key();
                                        let modifiers = evt.modifiers();

                                        // NOTE: on macOS, Meta is the Cmd key. On Windows/Linux,
                                        // Ctrl is used for shortcuts — both are checked here for
                                        // cross-platform consistency. The blitz-shell maps macOS
                                        // Cmd → Modifiers::SUPER (not META), so we check SUPER too.
                                        if modifiers.ctrl()
                                            || modifiers.meta()
                                            || modifiers.contains(Modifiers::SUPER)
                                        {
                                            match &key {
                                                Key::Character(ch) => match ch.as_str() {
                                                    "a" => {
                                                        // Select all: anchor at document start, focus at end.
                                                        let layout_opt = {
                                                            let state = doc_state
                                                                .lock()
                                                                .unwrap_or_else(|e| e.into_inner());
                                                            state.paginated_layout.clone()
                                                        };
                                                        if let Some(layout) = layout_opt {
                                                            let first = DocumentPosition {
                                                                page_index: 0,
                                                                paragraph_index: 0,
                                                                byte_offset: 0,
                                                            };
                                                            // Find the last block on the last page.
                                                            let last_opt = layout
                                                                .pages
                                                                .iter()
                                                                .enumerate()
                                                                .rev()
                                                                .find_map(|(pi, page)| {
                                                                    page.editing_data
                                                                        .as_ref()?
                                                                        .paragraphs
                                                                        .iter()
                                                                        .max_by_key(|p| p.block_index)
                                                                        .map(|p| (pi, p.block_index))
                                                                });
                                                            if let Some((last_page, last_block)) = last_opt {
                                                                let end_offset = loro_doc
                                                                    .read()
                                                                    .as_ref()
                                                                    .map(|l| get_block_text(l, last_block).len())
                                                                    .unwrap_or(0);
                                                                let last = DocumentPosition {
                                                                    page_index: last_page,
                                                                    paragraph_index: last_block,
                                                                    byte_offset: end_offset,
                                                                };
                                                                let mut cs = cursor_state.write();
                                                                cs.anchor = Some(first);
                                                                cs.focus = Some(last);
                                                            }
                                                        }
                                                    }
                                                    _ => {} // unknown Cmd+key — do nothing
                                                },
                                                _ => {} // Cmd+non-character — do nothing
                                            }
                                            return;
                                        }

                                        let focus = cursor_state.read().focus.clone();
                                        let Some(focus) = focus else { return; };

                                        match &key {
                                            // ── Printable characters ───────────────────────────
                                            Key::Character(ch) => {
                                                let ch = ch.clone();

                                                {
                                                    let ldoc_guard = loro_doc.read();
                                                    let Some(ldoc) = ldoc_guard.as_ref() else { return; };
                                                    if insert_text(ldoc, focus.paragraph_index, focus.byte_offset, &ch).is_err() {
                                                        return;
                                                    }
                                                }

                                                {
                                                    let ldoc_guard = loro_doc.read();
                                                    let Some(ldoc) = ldoc_guard.as_ref() else { return; };
                                                    apply_mutation_and_relayout(&doc_state, ldoc);
                                                }

                                                let new_offset = focus.byte_offset + ch.len();
                                                let new_pos = DocumentPosition {
                                                    byte_offset: new_offset,
                                                    ..focus
                                                };
                                                let mut cs = cursor_state.write();
                                                cs.focus = Some(new_pos.clone());
                                                cs.anchor = Some(new_pos);
                                            }

                                            // ── Backspace ──────────────────────────────────────
                                            Key::Backspace => {
                                                if focus.byte_offset == 0 {
                                                    if focus.paragraph_index == 0 {
                                                        return;
                                                    }
                                                    let ldoc_guard = loro_doc.read();
                                                    let Some(ldoc) = ldoc_guard.as_ref() else { return; };
                                                    let Ok(merged_offset) =
                                                        merge_block(ldoc, focus.paragraph_index)
                                                    else {
                                                        return;
                                                    };
                                                    apply_mutation_and_relayout(&doc_state, ldoc);
                                                    // TODO(3b-3): recompute page_index from layout after merge
                                                    let new_pos = DocumentPosition {
                                                        page_index: focus.page_index,
                                                        paragraph_index: focus.paragraph_index - 1,
                                                        byte_offset: merged_offset,
                                                    };
                                                    let mut cs = cursor_state.write();
                                                    cs.focus = Some(new_pos.clone());
                                                    cs.anchor = Some(new_pos);
                                                    return;
                                                }

                                                let text = {
                                                    let ldoc_guard = loro_doc.read();
                                                    ldoc_guard
                                                        .as_ref()
                                                        .map(|l| get_block_text(l, focus.paragraph_index))
                                                        .unwrap_or_default()
                                                };
                                                let prev = prev_grapheme_boundary(&text, focus.byte_offset);
                                                let len = focus.byte_offset - prev;

                                                {
                                                    let ldoc_guard = loro_doc.read();
                                                    let Some(ldoc) = ldoc_guard.as_ref() else { return; };
                                                    if delete_text(ldoc, focus.paragraph_index, prev, len).is_err() {
                                                        return;
                                                    }
                                                }

                                                {
                                                    let ldoc_guard = loro_doc.read();
                                                    let Some(ldoc) = ldoc_guard.as_ref() else { return; };
                                                    apply_mutation_and_relayout(&doc_state, ldoc);
                                                }

                                                let new_pos = DocumentPosition {
                                                    byte_offset: prev,
                                                    ..focus
                                                };
                                                let mut cs = cursor_state.write();
                                                cs.focus = Some(new_pos.clone());
                                                cs.anchor = Some(new_pos);
                                            }

                                            // ── Forward delete ─────────────────────────────────
                                            Key::Delete => {
                                                let text = {
                                                    let ldoc_guard = loro_doc.read();
                                                    ldoc_guard
                                                        .as_ref()
                                                        .map(|l| get_block_text(l, focus.paragraph_index))
                                                        .unwrap_or_default()
                                                };
                                                if focus.byte_offset >= text.len() {
                                                    return;
                                                }
                                                let next = next_grapheme_boundary(&text, focus.byte_offset);
                                                let len = next - focus.byte_offset;

                                                {
                                                    let ldoc_guard = loro_doc.read();
                                                    let Some(ldoc) = ldoc_guard.as_ref() else { return; };
                                                    if delete_text(
                                                        ldoc,
                                                        focus.paragraph_index,
                                                        focus.byte_offset,
                                                        len,
                                                    )
                                                    .is_err()
                                                    {
                                                        return;
                                                    }
                                                }

                                                {
                                                    let ldoc_guard = loro_doc.read();
                                                    let Some(ldoc) = ldoc_guard.as_ref() else { return; };
                                                    apply_mutation_and_relayout(&doc_state, ldoc);
                                                }
                                                // Cursor stays at the same offset after forward delete.
                                            }

                                            // ── Arrow-key navigation ───────────────────────────
                                            Key::ArrowLeft | Key::ArrowRight => {
                                                let shift_held = modifiers.shift();
                                                let layout_opt = {
                                                    let state = doc_state
                                                        .lock()
                                                        .unwrap_or_else(|e| e.into_inner());
                                                    state.paginated_layout.clone()
                                                };
                                                let Some(layout) = layout_opt else { return; };
                                                let ldoc_guard = loro_doc.read();
                                                let new_pos = if key == Key::ArrowLeft {
                                                    navigate_left(&focus, &layout, |idx| {
                                                        ldoc_guard
                                                            .as_ref()
                                                            .map(|l| get_block_text(l, idx))
                                                            .unwrap_or_default()
                                                    })
                                                } else {
                                                    navigate_right(&focus, &layout, |idx| {
                                                        ldoc_guard
                                                            .as_ref()
                                                            .map(|l| get_block_text(l, idx))
                                                            .unwrap_or_default()
                                                    })
                                                };
                                                if let Some(np) = new_pos {
                                                    let mut cs = cursor_state.write();
                                                    cs.focus = Some(np.clone());
                                                    if !shift_held {
                                                        cs.anchor = Some(np);
                                                    }
                                                }
                                            }

                                            Key::ArrowUp | Key::ArrowDown => {
                                                let shift_held = modifiers.shift();
                                                let layout_opt = {
                                                    let state = doc_state
                                                        .lock()
                                                        .unwrap_or_else(|e| e.into_inner());
                                                    state.paginated_layout.clone()
                                                };
                                                let Some(layout) = layout_opt else { return; };
                                                let new_pos = if key == Key::ArrowUp {
                                                    navigate_up(&focus, &layout)
                                                } else {
                                                    navigate_down(&focus, &layout)
                                                };
                                                if let Some(np) = new_pos {
                                                    let mut cs = cursor_state.write();
                                                    cs.focus = Some(np.clone());
                                                    if !shift_held {
                                                        cs.anchor = Some(np);
                                                    }
                                                }
                                            }

                                            Key::Home | Key::End => {
                                                let shift_held = modifiers.shift();
                                                let layout_opt = {
                                                    let state = doc_state
                                                        .lock()
                                                        .unwrap_or_else(|e| e.into_inner());
                                                    state.paginated_layout.clone()
                                                };
                                                let Some(layout) = layout_opt else { return; };
                                                let ldoc_guard = loro_doc.read();
                                                let new_pos = if key == Key::Home {
                                                    navigate_home(&focus, &layout)
                                                } else {
                                                    navigate_end(&focus, &layout, |idx| {
                                                        ldoc_guard
                                                            .as_ref()
                                                            .map(|l| get_block_text(l, idx))
                                                            .unwrap_or_default()
                                                    })
                                                };
                                                if let Some(np) = new_pos {
                                                    let mut cs = cursor_state.write();
                                                    cs.focus = Some(np.clone());
                                                    if !shift_held {
                                                        cs.anchor = Some(np);
                                                    }
                                                }
                                            }

                                            // ── Enter — split paragraph ─────────────────────────
                                            Key::Enter => {
                                                let ldoc_guard = loro_doc.read();
                                                let Some(ldoc) = ldoc_guard.as_ref() else { return; };
                                                if split_block(ldoc, focus.paragraph_index, focus.byte_offset)
                                                    .is_err()
                                                {
                                                    return;
                                                }
                                                apply_mutation_and_relayout(&doc_state, ldoc);
                                                // TODO(3b-3): recompute page_index from layout after split
                                                let new_pos = DocumentPosition {
                                                    page_index: focus.page_index,
                                                    paragraph_index: focus.paragraph_index + 1,
                                                    byte_offset: 0,
                                                };
                                                let mut cs = cursor_state.write();
                                                cs.focus = Some(new_pos.clone());
                                                cs.anchor = Some(new_pos);
                                            }

                                            _ => {}
                                        }
                                    }
                                },
                            }
                        }
                    },
                }
            }

            // ── Bottom status bar (flex-shrink: 0) ────────────────────────────
            AtStatusBar {
                page_label:          "Page 1 of 1".to_string(),
                // TODO(word-count): wire to actual document word count.
                word_count_label:    "".to_string(),
                // TODO(language): wire to document language setting.
                language_label:      "English (US)".to_string(),
                zoom_percent:        100,
                collaborator_count:  0,
                collaborator_label:  "".to_string(),
                on_zoom_click:       |_| {},
                zoom_aria_label:     "Zoom level",
            }
        }
    }
}

// ── Loading pipeline ──────────────────────────────────────────────────────────

/// Detected document format, derived from the file extension in the token's
/// display name.
enum DocumentFormat {
    Docx,
    Odt,
    Unsupported(String),
}

/// Inspect the display name on `token` and return the [`DocumentFormat`] for
/// this file.  The extension comparison is case-insensitive.
fn detect_format(token: &FileAccessToken) -> DocumentFormat {
    match token
        .display_name()
        .rsplit('.')
        .next()
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("docx") => DocumentFormat::Docx,
        Some("odt") => DocumentFormat::Odt,
        Some(ext) => DocumentFormat::Unsupported(ext.to_string()),
        None => DocumentFormat::Unsupported(String::new()),
    }
}

/// Deserialise `path` → detect format → open file → import → return
/// [`Document`].
///
/// Format is determined from the file extension in the
/// [`FileAccessToken`] display name before the file is opened, so the
/// reader is only consumed once.  All I/O is synchronous; this function is
/// called inside an `async move` block in [`use_resource`] so that loading
/// does not block the initial render of the editor shell.
fn load_document(path: String) -> Result<Document, LoadError> {
    let token = FileAccessToken::deserialize(&path)?;
    let format = detect_format(&token);
    let reader = token.open_read()?;
    let doc = match format {
        DocumentFormat::Docx => {
            DocxImport::import(reader, DocxImportOptions::default()).map_err(LoadError::Ooxml)?
            // TODO(odt-fidelity): DOCX rendering gaps (styles, page size) tracked separately.
        }
        DocumentFormat::Odt => {
            OdtImport::import(reader, OdtImportOptions::default()).map_err(LoadError::Odt)?
            // TODO(odt-fidelity): ODT rendering gaps — some paragraph styles, list
            // indents, and image placement may not render correctly yet.
        }
        DocumentFormat::Unsupported(ext) => {
            return Err(LoadError::UnsupportedFormat(ext));
        }
    };
    Ok(doc)
}
