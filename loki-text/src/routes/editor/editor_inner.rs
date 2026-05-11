// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Document editor — inner component.
//!
//! [`EditorInner`] holds all per-document hook state and renders the
//! three-panel editor layout: top toolbar and scrollable page canvas.
//! The persistent tab bar and status bar live in [`crate::routes::shell::Shell`].
//!
//! This component is keyed on the document `path` by [`super::Editor`],
//! so every tab switch triggers a clean remount, ensuring fresh hook state
//! (document load, GPU surface, Loro bridge, cursor) per document.

use std::sync::Arc;

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::document::Document;
use loki_doc_model::loro_bridge::{derive_loro_cursor, document_to_loro};
use loki_layout::LayoutOptions;

use crate::components::toolbar::TopToolbar;
use crate::components::wgpu_surface::WgpuSurface;
use crate::editing::cursor::DocumentPosition;
use crate::editing::touch::TouchInteractionState;
use crate::error::LoadError;
use crate::utils::display_title_from_path;

use super::EditorMode;
use super::editor_error_view::EditorErrorView;
use super::editor_keydown::make_keydown_handler;
use super::editor_load::load_document;
use super::editor_pointer::{
    make_mousemove_handler, make_touchend_handler, make_touchmove_handler,
};
use super::editor_state::{EditorState, use_editor_state};

/// Document editor inner component — all editing logic lives here.
///
/// [`super::Editor`] renders this with `key: "{path}"` so a tab switch causes
/// a full remount, giving each document clean hook state.
#[component]
pub(super) fn EditorInner(path: String) -> Element {
    let title = display_title_from_path(&path);

    let EditorState {
        doc_state,
        editor_mode,
        mut loro_doc,
        mut cursor_state,
        mut is_dragging,
        mut drag_origin,
        touch_state,
        window_width,
        scroll_offset,
    } = use_editor_state();

    // Pre-clone the Arc so each closure can capture its own owned clone.
    let doc_state_mousemove = Arc::clone(&doc_state);
    let doc_state_touch = Arc::clone(&doc_state);
    let doc_state_touchend = Arc::clone(&doc_state);
    let doc_state_prop = Arc::clone(&doc_state);
    let doc_state_keydown = Arc::clone(&doc_state);

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
    //   When scroll_offset_y ≥ container_y (= TOOLBAR_HEIGHT_TOP), the scroll
    //   container claims every click including those in the toolbar row.
    //   pointer-events:none is NOT implemented in this Blitz version.
    //
    // FIX: TopToolbar carries `position: relative; z-index: 10` (see toolbar.rs).
    //   Blitz hit-tests paint_children in reverse z_index order
    //   [blitz-dom-0.2.4/src/layout/damage.rs:353-383], so TopToolbar wins.

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

            // ── Top toolbar (flex-shrink: 0) ───────────────────────────────────
            TopToolbar {
                title: title,
                editor_mode: editor_mode
            }

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

                    Some(Err(e)) => {
                        let msg = e.to_string();
                        rsx! { EditorErrorView { message: msg } }
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
                }
            }
        }
    }
}
