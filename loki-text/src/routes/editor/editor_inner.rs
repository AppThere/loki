// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Document editor — inner component.
//!
//! [`EditorInner`] holds all per-document hook state and renders the editor
//! layout: ribbon, scrollable page canvas, and status bar.
//!
//! ## Reactive document switching (Pass 7)
//!
//! `EditorInner` is **not** remounted on tab switch — `key` on a single
//! non-list component is a no-op in Dioxus 0.7.  Instead, document switching
//! is handled reactively:
//!
//! 1. `path_signal` is a `Signal<String>` kept in sync with the `path` prop
//!    via synchronous comparison each render.
//! 2. `use_resource` reads `path_signal()` so the load task is cancelled and
//!    restarted whenever the active document changes.
//! 3. All per-document state is reset synchronously when path changes so the
//!    reset happens before `use_resource` evaluates.

use std::sync::Arc;

use appthere_ui::tokens;
use appthere_ui::{AtRibbon, AtStatusBar, RibbonTabDesc};
use dioxus::prelude::*;
use loki_doc_model::document::Document;
use loki_doc_model::get_mark_at;
use loki_doc_model::loro_bridge::document_to_loro;
use loki_doc_model::loro_schema::{
    MARK_BOLD, MARK_ITALIC, MARK_STRIKETHROUGH, MARK_UNDERLINE, MARK_VERTICAL_ALIGN,
};
use loki_i18n::fl;
use loki_layout::LayoutOptions;
use loro::LoroValue;

use super::editor_canvas::render_canvas_area;
use super::editor_load::load_document;
use super::editor_ribbon::home_tab_content;
use super::editor_state::{EditorState, use_editor_state};
use crate::error::LoadError;

// EditorMode removed — the editor is always in edit mode when a document is
// open. Distraction-free reading is handled by the View ribbon tab (future
// pass), not by a separate mode.

/// Document editor inner component — all editing logic lives here.
///
/// Document switching is handled reactively via `path_signal` — see the
/// module-level doc for the full design.
#[component]
pub(super) fn EditorInner(path: String) -> Element {
    // ── Path signal: bridge from prop-space to signal-space ──────────────────
    let mut path_signal: Signal<String> = use_signal(|| path.clone());

    let EditorState {
        doc_state,
        mut loro_doc,
        mut cursor_state,
        is_dragging,
        drag_origin,
        touch_state,
        window_width,
        scroll_offset,
        mut current_page,
        mut total_pages,
        mut bold_active,
        mut italic_active,
        mut underline_active,
        mut strikethrough_active,
        mut superscript_active,
        mut subscript_active,
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

            if let Ok(mut state) = doc_state.lock() {
                state.document = None;
                state.generation = 0;
                state.page_count = 0;
                state.canvas_width = 0.0;
                state.visible_rect = None;
                state.paginated_layout = None;
                state.layout_stamp = state.layout_stamp.wrapping_add(1);
                state.layout_generation = 0;
                state.layout_canvas_width = 0.0;
                state.layout_preserve_for_editing = false;
            } else {
                tracing::error!("doc_state lock poisoned during tab switch — state may be stale");
            }

            cursor_state.set(crate::editing::cursor::CursorState::default());
            loro_doc.set(None);
            total_pages.set(0);
            current_page.set(1);
        }
    }

    // Pre-clone the Arc so each closure can capture its own owned clone.
    let doc_state_mousemove = Arc::clone(&doc_state);
    let doc_state_touch = Arc::clone(&doc_state);
    let doc_state_touchend = Arc::clone(&doc_state);
    let doc_state_prop = Arc::clone(&doc_state);
    let doc_state_keydown = Arc::clone(&doc_state);
    let doc_state_pages = Arc::clone(&doc_state);
    let doc_state_ribbon = Arc::clone(&doc_state);

    // ── Document load — reactive on path_signal ───────────────────────────────
    let document_load: Resource<(String, Result<Document, LoadError>)> = use_resource(move || {
        let p = path_signal();
        async move {
            let res = load_document(p.clone());
            (p, res)
        }
    });

    let _navigator = use_navigator();

    // ── Loro bridge: initialise CRDT once the document is loaded ─────────────
    use_effect(move || {
        if let Some((loaded_path, Ok(doc))) = &*document_load.value().read_unchecked()
            && loaded_path == &path_signal()
            && loro_doc().is_none()
        {
            match document_to_loro(doc) {
                Ok(l_doc) => loro_doc.set(Some(l_doc)),
                Err(e) => tracing::warn!("Failed to initialize Loro sync bridge: {}", e),
            }
        }
    });

    // ── Page count sync — re-runs when document_load resolves ────────────────
    //
    // Subscribe to `document_load.value()` so this effect re-runs when the
    // resource resolves.  WgpuSurface renders synchronously as part of the
    // same render cycle that changed the resource signal, so by the time this
    // post-render effect fires, doc_state.page_count is already updated.
    use_effect(move || {
        // Reactive read — subscribes so this effect re-runs when the document
        // finishes loading (resource signal changes).
        let resource_signal = document_load.value();
        let _sub = resource_signal.read();
        if let Ok(state) = doc_state_pages.lock() {
            let count = state.page_count as u32;
            if *total_pages.peek() != count {
                total_pages.set(count);
            }
        }
    });

    // ── Inline formatting signal sync ────────────────────────────────────────
    //
    // Subscribes to cursor_state and loro_doc so this effect re-runs whenever
    // the cursor moves or the document changes. Updates the ribbon button
    // active states to reflect the marks at the focus position.
    use_effect(move || {
        let cs = cursor_state.read();
        let ldoc_guard = loro_doc.read();
        if let (Some(ldoc), Some(focus)) = (ldoc_guard.as_ref(), cs.focus.as_ref()) {
            let bi = focus.paragraph_index;
            let bo = focus.byte_offset;
            let is_bool = |key: &str| {
                matches!(
                    get_mark_at(ldoc, bi, bo, key),
                    Ok(Some(LoroValue::Bool(true)))
                )
            };
            bold_active.set(is_bool(MARK_BOLD));
            italic_active.set(is_bool(MARK_ITALIC));
            underline_active.set(is_bool(MARK_UNDERLINE));
            strikethrough_active.set(is_bool(MARK_STRIKETHROUGH));
            superscript_active.set(matches!(
                get_mark_at(ldoc, bi, bo, MARK_VERTICAL_ALIGN),
                Ok(Some(LoroValue::String(ref s))) if s.as_str() == "Superscript"
            ));
            subscript_active.set(matches!(
                get_mark_at(ldoc, bi, bo, MARK_VERTICAL_ALIGN),
                Ok(Some(LoroValue::String(ref s))) if s.as_str() == "Subscript"
            ));
        } else {
            bold_active.set(false);
            italic_active.set(false);
            underline_active.set(false);
            strikethrough_active.set(false);
            superscript_active.set(false);
            subscript_active.set(false);
        }
    });

    // ── Current page from scroll offset ──────────────────────────────────────
    //
    // TODO(scroll): current_page update on scroll is blocked — Blitz native's
    // convert_scroll_data is unimplemented! (panics at runtime), so onscroll
    // handlers cannot be used safely.  current_page stays at 1 until Blitz
    // exposes a scroll-position hook.  See:
    //   patches/dioxus-native-dom/src/events.rs convert_scroll_data
    let _ = scroll_offset;

    let layout_opts = LayoutOptions {
        // EditorMode removed — always preserve editing layout when a document
        // is open. Distraction-free reading via View ribbon tab (future pass).
        preserve_for_editing: true,
    };

    let page_gap_px = tokens::PAGE_GAP_PX;

    let page_label = if total_pages() == 0 {
        fl!("editor-page-loading") // empty in en-US — avoids flash while loading
    } else {
        fl!(
            "editor-page-label",
            current = current_page() as i64,
            total = total_pages() as i64
        )
    };

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; flex: 1; \
                 overflow: hidden; background: {bg}; font-family: system-ui, sans-serif;",
                bg = tokens::COLOR_SURFACE_BASE,
            ),

            // ── Scrollable page canvas ────────────────────────────────────────
            {render_canvas_area(
                doc_state_mousemove,
                doc_state_touch,
                doc_state_touchend,
                doc_state_prop,
                doc_state_keydown,
                is_dragging,
                drag_origin,
                touch_state,
                window_width,
                scroll_offset,
                cursor_state,
                loro_doc,
                path_signal,
                layout_opts,
                document_load,
                page_gap_px,
            )}

            // ── Ribbon (formatting controls) ──────────────────────────────────
            AtRibbon {
                tabs: vec![
                    RibbonTabDesc { label: fl!("ribbon-tab-home"),   is_contextual: false, aria_label: None },
                    RibbonTabDesc { label: fl!("ribbon-tab-insert"), is_contextual: false, aria_label: None },
                    RibbonTabDesc { label: fl!("ribbon-tab-format"), is_contextual: false, aria_label: None },
                    RibbonTabDesc { label: fl!("ribbon-tab-review"), is_contextual: false, aria_label: None },
                    RibbonTabDesc { label: fl!("ribbon-tab-view"),   is_contextual: false, aria_label: None },
                ],
                active_tab: 0,
                on_tab_select: move |_idx| {
                    // TODO(ribbon): Wire ribbon tab selection to per-document state.
                },
                tab_content: home_tab_content(
                    &doc_state_ribbon,
                    loro_doc,
                    cursor_state,
                    bold_active,
                    italic_active,
                    underline_active,
                    strikethrough_active,
                    superscript_active,
                    subscript_active,
                ),
            }

            // ── Status bar ────────────────────────────────────────────────────
            AtStatusBar {
                page_label:         page_label,
                word_count_label:   "".to_string(),
                language_label:     fl!("editor-language"),
                zoom_percent:       100,
                collaborator_count: 0,
                collaborator_label: String::new(),
                zoom_aria_label:    fl!("editor-zoom-aria"),
                on_zoom_click:      |_| {},
            }
        }
    }
}
