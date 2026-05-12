// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Document editor — inner component.
//!
//! [`EditorInner`] holds all per-document hook state and renders the
//! editor layout: scrollable page canvas.
//! The persistent tab bar and status bar live in [`crate::routes::shell::Shell`].
//!
//! ## Reactive document switching (Pass 7)
//!
//! `EditorInner` is **not** remounted on tab switch — `key` on a single
//! non-list component is a no-op in Dioxus 0.7.  Instead, document switching
//! is handled reactively:
//!
//! 1. `path_signal` is a `Signal<String>` kept in sync with the `path` prop
//!    via `use_effect`.  Because it is a signal, downstream hooks can truly
//!    subscribe to it.
//! 2. `use_resource` reads `path_signal()` so the load task is cancelled and
//!    restarted whenever the active document changes.
//! 3. A second `use_effect` subscribes to `path_signal` and resets all
//!    per-document state (`doc_state`, `cursor_state`, `loro_doc`,
//!    `editor_mode`) whenever the path changes — **but only when it actually
//!    changes**, using a previous-value guard so the effect is a no-op on the
//!    initial render (where there is nothing to reset).

use std::sync::Arc;

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{derive_loro_cursor, document_to_loro};
use loki_layout::LayoutOptions;

use super::EditorMode;
use super::editor_error_view::EditorErrorView;
use super::editor_keydown::make_keydown_handler;
use super::editor_load::load_document;
use super::editor_pointer::{
    make_mousemove_handler, make_touchend_handler, make_touchmove_handler,
};
use super::editor_state::{EditorState, use_editor_state};
use crate::components::wgpu_surface::WgpuSurface;
use crate::editing::cursor::{CursorState, DocumentPosition};
use crate::editing::touch::TouchInteractionState;
use crate::error::LoadError;

/// Document editor inner component — all editing logic lives here.
///
/// Document switching is handled reactively via `path_signal` — see the
/// module-level doc for the full design.
#[component]
pub(super) fn EditorInner(path: String) -> Element {
    // ── Path signal: bridge from prop-space to signal-space ──────────────────
    //
    // `path` is a plain `String` prop — not a signal — so `use_memo` cannot
    // make it reactive (it captures the value at mount time and never changes).
    // Instead, we hold the current path in a `Signal<String>` and sync it from
    // the prop on every render via `use_effect`.  Because `use_effect` fires
    // after every render, `path_signal` stays in sync with the prop, and any
    // hook that reads `path_signal()` subscribes reactively to path changes.
    let mut path_signal: Signal<String> = use_signal(|| path.clone());

    let EditorState {
        doc_state,
        mut editor_mode,
        mut loro_doc,
        mut cursor_state,
        mut is_dragging,
        mut drag_origin,
        touch_state,
        window_width,
        scroll_offset,
    } = use_editor_state();

    // ── Synchronous Path Sync & State Reset ──────────────────────────────────
    //
    // Sync the signal with the prop synchronously during the render.
    // By resetting the state here, we guarantee it happens BEFORE `use_resource`
    // evaluates its closure or restarts, and definitely BEFORE `WgpuSurface`
    // receives the new document. This strictly prevents the race condition where
    // a deferred `use_effect` runs late and wipes out the newly loaded document.
    {
        let current = path_signal.peek().clone();
        if current != path {
            tracing::debug!(
                "EditorInner: path changed from {} to {} → resetting per-document state",
                current,
                path
            );
            path_signal.set(path.clone());

            // Clear the Mutex-protected doc_state fields. WgpuSurface will detect
            // document == None and show the placeholder.
            if let Ok(mut state) = doc_state.lock() {
                state.document = None;
                state.generation = 0;
                state.page_count = 0;
                state.canvas_width = 0.0;
                state.visible_rect = None;
                state.paginated_layout = None;
                // Bump layout_stamp so any in-flight LokiDocumentSource render()
                // call sees a mismatch against its cached texture_stamp and
                // recomputes rather than returning a stale texture.
                state.layout_stamp = state.layout_stamp.wrapping_add(1);
                state.layout_generation = 0;
                state.layout_canvas_width = 0.0;
                state.layout_preserve_for_editing = false;
            } else {
                tracing::error!("doc_state lock poisoned during tab switch — state may be stale");
            }

            // Reset per-document signals.
            cursor_state.set(CursorState::default());
            loro_doc.set(None);
            editor_mode.set(EditorMode::Reading);
        }
    }

    // Pre-clone the Arc so each closure can capture its own owned clone.
    let doc_state_mousemove = Arc::clone(&doc_state);
    let doc_state_touch = Arc::clone(&doc_state);
    let doc_state_touchend = Arc::clone(&doc_state);
    let doc_state_prop = Arc::clone(&doc_state);
    let doc_state_keydown = Arc::clone(&doc_state);

    // ── Document load — reactive on path_signal ───────────────────────────────
    //
    // `path_signal()` is a reactive read: when the signal changes (path prop
    // changes on tab switch), Dioxus invalidates the resource and restarts the
    // async task with the new path value.
    let document_load: Resource<(String, Result<Document, LoadError>)> = use_resource(move || {
        let p = path_signal(); // reactive read — resource restarts when path changes
        async move {
            let res = load_document(p.clone());
            (p, res)
        }
    });

    let _navigator = use_navigator();

    // ── Reset per-document state when path changes ────────────────────────────
    //

    // ── Loro bridge: initialise CRDT once the document is loaded ─────────────
    use_effect(move || {
        if let Some((loaded_path, Ok(doc))) = &*document_load.value().read_unchecked() {
            if loaded_path == &path_signal() && loro_doc().is_none() {
                match document_to_loro(doc) {
                    Ok(l_doc) => loro_doc.set(Some(l_doc)),
                    Err(e) => tracing::warn!("Failed to initialize Loro sync bridge: {}", e),
                }
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
    //   When scroll_offset_y ≥ container_y (= TOOLBAR_HEIGHT_TOP), the scroll
    //   container claims every click including those in the toolbar row.
    //   pointer-events:none is NOT implemented in this Blitz version.
    //
    // NOTE: TopToolbar (now removed) used z-index: 10 to win Blitz hit tests.
    //   The scroll container now owns the full editor area; the ribbon is in
    //   the Shell and does not overlap the scroll container.

    rsx! {
        div {
            // AtTabBar and AtStatusBar live in Shell (routes/shell.rs) and
            // persist across route transitions. Editor fills the flex space
            // allocated by Shell's Outlet wrapper.
            style: format!(
                "display: flex; flex-direction: column; flex: 1; \
                 overflow: hidden; background: {bg}; font-family: system-ui, sans-serif;",
                bg = tokens::COLOR_SURFACE_BASE,
            ),

            // TopToolbar removed — replaced by AtRibbon in the Shell layout.
            // Formatting controls will be implemented as AtRibbonGroup content
            // in a future pass.

            // ── Scroll container ──────────────────────────────────────────────
            //
            // COMPAT(dioxus-native): flex: 1 is confirmed working. Requires
            // height: 100vh on the parent so Taffy can resolve the flex fraction.
            div {
                style: format!(
                    "flex: 1; min-height: 0; overflow-y: auto; overflow-x: hidden; \
                     background: {bg}; padding: {p}px 0;",
                    bg = tokens::COLOR_SURFACE_BASE,
                    p  = tokens::SPACE_6,
                ),

                onmousedown: move |evt| {
                    let c = evt.client_coordinates();
                    drag_origin.set(Some((c.x as f32, c.y as f32)));
                },

                onmousemove: make_mousemove_handler(
                    doc_state_mousemove,
                    is_dragging,
                    editor_mode,
                    drag_origin,
                    window_width,
                    scroll_offset,
                    cursor_state,
                    page_gap_px,
                ),

                onmouseup: move |_| {
                    is_dragging.set(false);
                    drag_origin.set(None);
                },

                ontouchstart: move |evt: TouchEvent| {
                    if editor_mode() != EditorMode::Editing { return; }
                    let touches = evt.touches();
                    let Some(first) = touches.first() else { return };
                    let c = first.client_coordinates();
                    touch_state.clone().set(Some(TouchInteractionState::new(0, (c.x as f32, c.y as f32))));
                },

                ontouchmove: make_touchmove_handler(
                    doc_state_touch,
                    editor_mode,
                    touch_state.clone(),
                    window_width,
                    scroll_offset,
                    loro_doc,
                    cursor_state,
                    page_gap_px,
                ),

                ontouchend: make_touchend_handler(
                    doc_state_touchend,
                    editor_mode,
                    touch_state,
                    window_width,
                    scroll_offset,
                    loro_doc,
                    cursor_state,
                    page_gap_px,
                ),

                match &*document_load.value().read_unchecked() {
                    Some((loaded_path, Ok(doc))) if loaded_path == &path_signal() => {
                        let cs = if editor_mode() == EditorMode::Editing {
                            Some(cursor_state.read().clone())
                        } else {
                            None
                        };
                        rsx! {
                            WgpuSurface {
                                doc_state: Arc::clone(&doc_state_prop),
                                path: path_signal(),
                                document: Some(doc.clone()),
                                layout_opts: layout_opts.clone(),
                                visible_rect: None,
                                cursor_state: cs,
                                on_mousedown: move |p: DocumentPosition| {
                                    if editor_mode() != EditorMode::Editing { return; }
                                    is_dragging.set(true);
                                    let loro_cursor = loro_doc.read().as_ref().and_then(|ldoc| {
                                        derive_loro_cursor(ldoc, p.paragraph_index, p.byte_offset)
                                    });
                                    let mut cs = cursor_state.write();
                                    cs.loro_cursor = loro_cursor;
                                    cs.anchor = Some(p.clone());
                                    cs.focus = Some(p);
                                },
                                on_keydown: make_keydown_handler(
                                    doc_state_keydown,
                                    editor_mode,
                                    cursor_state,
                                    loro_doc,
                                ),
                            }
                        }
                    },

                    Some((loaded_path, Err(e))) if loaded_path == &path_signal() => {
                        let msg = e.to_string();
                        rsx! { EditorErrorView { message: msg } }
                    },

                    // Covers `None` and any stale resource states where
                    // loaded_path != path_signal()
                    _ => rsx! {
                        WgpuSurface {
                            doc_state: Arc::clone(&doc_state_prop),
                            path: path_signal(),
                            document: None,
                            layout_opts: layout_opts.clone(),
                            visible_rect: None,
                            cursor_state: None,
                            on_mousedown: |_| {},
                            on_keydown: |_| {},
                        }
                    },
                }
            }
        }
    }
}
