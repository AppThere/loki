// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

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
use loki_renderer::ViewMode;
use loro::LoroValue;

use super::editor_canvas::render_canvas_area;
use super::editor_load::load_document;
use super::editor_metadata_panel::metadata_panel;
use super::editor_path_sync::{
    PathSyncSignals, restore_session, stash_outgoing, sync_path_and_reset,
};
use super::editor_publish::{publish_panel, publish_tab_content};
use super::editor_ribbon::home_tab_content;
use super::editor_save::{export_document_to_token, save_document_to_path};
use super::editor_state::{EditorState, use_editor_state};
use super::editor_style::style_picker_panel;
use super::editor_style_editor::style_editor_panel;
use crate::error::LoadError;
use crate::new_document::is_untitled;
use crate::recent_documents::RecentDocuments;
use crate::routes::Route;
use crate::sessions::DocSessions;
use crate::tabs::OpenTab;
use crate::utils::display_title_from_path;
use loki_file_access::{FilePicker, SaveOptions};

/// MIME type used when saving documents (DOCX is the only writable format).
const DOCX_MIME: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

/// Viewport width (logical px) below which the editor defaults to the
/// reflowable view: a US-Letter page (~816px) plus margins no longer fits, so
/// paginated view would otherwise force horizontal scrolling. The user can
/// still toggle back to paginated.
const REFLOW_BREAKPOINT_PX: f32 = 900.0;

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

    // ── Font warning dismiss state ───────────────────────────────────────────
    let mut dismiss_font_warning = use_signal(|| false);

    // ── Ribbon collapse state ────────────────────────────────────────────────
    let mut ribbon_collapsed = use_signal(|| false);

    // ── Style search query (cleared on picker close) ─────────────────────────
    let style_search_query = use_signal(String::new);

    let EditorState {
        doc_state,
        mut loro_doc,
        mut cursor_state,
        is_dragging,
        drag_origin,
        touch_state,
        window_width,
        scroll_offset,
        scroll_metrics,
        canvas_mounted,
        vbar_drag,
        hbar_drag,
        current_page,
        mut total_pages,
        mut view_mode,
        mut view_mode_user_set,
        mut bold_active,
        mut italic_active,
        mut underline_active,
        mut strikethrough_active,
        mut superscript_active,
        mut subscript_active,
        mut undo_manager,
        can_undo,
        can_redo,
        is_style_picker_open,
        editing_style_draft,
        mut save_message,
        save_request,
        mut active_ribbon_tab,
        is_publish_panel_open,
        pdf_level,
        editing_metadata,
    } = use_editor_state();

    // ── Tab/recents context for Save As and the unsaved-changes indicator ────
    let mut tabs = use_context::<Signal<Vec<OpenTab>>>();
    let recent_docs = use_context::<Signal<RecentDocuments>>();
    // Stashed sessions for inactive tabs — unsaved edits survive tab switches.
    let doc_sessions = use_context::<Signal<DocSessions>>();
    // Document generation considered "clean" (matches the on-disk file).
    // Captured when the document finishes loading and after each successful
    // save; the tab is dirty whenever the live generation differs.
    let mut baseline_gen = use_signal(|| 0_u64);

    // ── Session restore at mount ─────────────────────────────────────────────
    //
    // Navigating Editor → Home unmounts this component (different routes), so
    // returning to a document tab mounts a fresh EditorInner. Restore the
    // stashed session here — before `use_resource` evaluates — so unsaved
    // edits survive the round trip. The matching stash happens in the
    // unmount hook below.
    {
        let doc_state_restore = Arc::clone(&doc_state);
        let mut sessions_at_mount = doc_sessions;
        use_hook(move || {
            let initial_path = path_signal.peek().clone();
            let restored = sessions_at_mount.write().remove(&initial_path);
            if let Some(session) = restored {
                let mut sig = PathSyncSignals {
                    cursor_state,
                    loro_doc,
                    undo_manager,
                    total_pages,
                    current_page,
                    can_undo,
                    can_redo,
                    dismiss_font_warning,
                    is_style_picker_open,
                    editing_style_draft,
                    save_message,
                    baseline_gen,
                };
                restore_session(session, &doc_state_restore, &mut sig);
            }
        });
    }

    // ── Session stash at unmount ─────────────────────────────────────────────
    //
    // Skipped when the tab was closed (Shell already dropped the session and
    // re-stashing would resurrect discarded edits on reopen).
    {
        let doc_state_drop = Arc::clone(&doc_state);
        let tabs_at_drop = tabs;
        let mut sessions_at_drop = doc_sessions;
        use_drop(move || {
            let path = path_signal.peek().clone();
            if !tabs_at_drop.peek().iter().any(|t| t.path == path) {
                return;
            }
            let mut sig = PathSyncSignals {
                cursor_state,
                loro_doc,
                undo_manager,
                total_pages,
                current_page,
                can_undo,
                can_redo,
                dismiss_font_warning,
                is_style_picker_open,
                editing_style_draft,
                save_message,
                baseline_gen,
            };
            stash_outgoing(&path, &doc_state_drop, &mut sessions_at_drop, &mut sig);
        });
    }

    // ── Synchronous Path Sync & Session Handover ─────────────────────────────
    //
    // Stashes the outgoing document's live state and restores (or resets) the
    // incoming document's state synchronously during the render phase so the
    // handover happens BEFORE `use_resource` evaluates.  See `editor_path_sync`.
    sync_path_and_reset(
        &path,
        &mut path_signal,
        &doc_state,
        doc_sessions,
        &mut PathSyncSignals {
            cursor_state,
            loro_doc,
            undo_manager,
            total_pages,
            current_page,
            can_undo,
            can_redo,
            dismiss_font_warning,
            is_style_picker_open,
            editing_style_draft,
            save_message,
            baseline_gen,
        },
    );

    // Compute the current paragraph style name directly from signals so it is
    // always up-to-date in the same render cycle as cursor movement.
    let current_style_name = {
        let cs = cursor_state.read();
        let ldoc = loro_doc.read();
        if let (Some(l), Some(focus)) = (ldoc.as_ref(), cs.focus.as_ref()) {
            loki_doc_model::get_block_style_name(l, focus.paragraph_index)
        } else {
            String::new()
        }
    };

    // Pre-clone the Arc so each closure can capture its own owned clone.
    let doc_state_mousedown = Arc::clone(&doc_state);
    let doc_state_mousemove = Arc::clone(&doc_state);
    let doc_state_touch = Arc::clone(&doc_state);
    let doc_state_touchend = Arc::clone(&doc_state);
    let doc_state_keydown = Arc::clone(&doc_state);
    let doc_state_pages = Arc::clone(&doc_state);
    let doc_state_ribbon = Arc::clone(&doc_state);
    let doc_state_publish = Arc::clone(&doc_state);
    let doc_state_publish_panel = Arc::clone(&doc_state);
    let doc_state_meta = Arc::clone(&doc_state);
    let doc_state_style_picker = Arc::clone(&doc_state);
    let doc_state_style_editor = Arc::clone(&doc_state);
    let doc_state_seed = Arc::clone(&doc_state);
    let doc_state_render = Arc::clone(&doc_state);
    let doc_state_scroll = Arc::clone(&doc_state);

    // ── Document load — reactive on path_signal ───────────────────────────────
    let document_load: Resource<(String, Result<Document, LoadError>)> = use_resource(move || {
        let p = path_signal();
        async move {
            let res = load_document(p.clone());
            (p, res)
        }
    });

    let navigator = use_navigator();

    // ── Loro bridge: initialise CRDT once the document is loaded ─────────────
    //
    // The first paginated layout is a CPU-heavy pass (tens of ms on a multi-page
    // document, because the shared font caches start cold). Running it inline in
    // this post-render effect blocks the frame, so the loading indicator never
    // paints and the open appears to freeze.
    //
    // Instead, lay out on a worker thread (`compute_layout_off_main_thread`) and
    // await the result: the main thread stays free to paint the loading
    // indicator (the canvas shows it until `total_pages > 0`, set at the end of
    // this task) and remains responsive. The guard `loro_doc().is_none()` is
    // only cleared at the end of the task, and no signal this effect subscribes
    // to changes while the worker runs, so the task is spawned once per open.
    use_effect(move || {
        if let Some((loaded_path, Ok(doc))) = &*document_load.value().read_unchecked()
            && loaded_path == &path_signal()
            && loro_doc().is_none()
        {
            let loaded_path = loaded_path.clone();
            let doc = doc.clone();
            let doc_state_seed = Arc::clone(&doc_state_seed);
            spawn(async move {
                // Lay out off the main thread; the await is a cross-thread yield.
                let Some((doc, layout)) =
                    super::editor_layout_task::compute_layout_off_main_thread(
                        Arc::clone(&doc_state_seed),
                        doc,
                    )
                    .await
                else {
                    return;
                };

                // The user may have switched tabs while the worker ran. If so,
                // `path_signal` now points at a different document whose state
                // was already reset/restored — publishing here would clobber it,
                // so bail out and discard the stale layout.
                if path_signal.peek().as_str() != loaded_path {
                    return;
                }

                // Seed the layout so hit-testing works on the very first click,
                // before any Loro mutation triggers apply_mutation_and_relayout.
                let page_count =
                    crate::editing::state::publish_seed_layout(&doc_state_seed, &doc, layout);

                match document_to_loro(&doc) {
                    Ok(l_doc) => {
                        let um = loro::UndoManager::new(&l_doc);
                        loro_doc.set(Some(l_doc));
                        undo_manager.set(Some(um));

                        // The freshly-loaded document matches the file on disk:
                        // record the current generation as the clean baseline.
                        baseline_gen.set(cursor_state.peek().document_generation);

                        // Auto-place the cursor at the start of the document so
                        // the user can type immediately without clicking first.
                        if cursor_state.read().focus.is_none() {
                            use crate::editing::cursor::DocumentPosition;
                            let start = DocumentPosition {
                                page_index: 0,
                                paragraph_index: 0,
                                byte_offset: 0,
                            };
                            let mut cs = cursor_state.write();
                            cs.anchor = Some(start.clone());
                            cs.focus = Some(start);
                        }

                        // Publish the page count now the layout is ready — this
                        // lifts the canvas's loading gate (total_pages > 0).
                        total_pages.set(page_count as u32);
                    }
                    Err(e) => tracing::warn!("Failed to initialize Loro sync bridge: {}", e),
                }
            });
        }
    });

    // ── Page count sync — re-runs when document_load resolves ────────────────
    //
    // Subscribe to `document_load.value()` so this effect re-runs when the
    // resource resolves.  By the time this post-render effect fires,
    // doc_state.page_count is already updated.
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

    // ── Inline formatting + style signal sync ────────────────────────────────
    //
    // Subscribes to cursor_state and loro_doc so this effect re-runs whenever
    // the cursor moves or the document changes. Updates the ribbon button
    // active states and the current paragraph style name.
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
    // Updated by the onscroll handler in editor_canvas.rs. Scroll events are
    // dispatched by the patched Blitz shell (PATCH(loki) in blitz-shell /
    // blitz-dom / dioxus-native-dom) whenever a wheel or touch gesture changes
    // the scroll container's offset.

    // ── Unsaved-changes (dirty) tracking ─────────────────────────────────────
    //
    // The tab shows a dirty indicator when the live document generation differs
    // from the clean baseline captured at load/save. Untitled documents are
    // always dirty until the first Save As. Runs whenever the cursor's mirrored
    // generation, the path, or the baseline changes.
    use_effect(move || {
        let live_gen = cursor_state.read().document_generation;
        let path = path_signal();
        let base = baseline_gen();
        let dirty = is_untitled(&path) || live_gen != base;
        let mut t = tabs.write();
        if let Some(tab) = t.iter_mut().find(|tab| tab.path == path)
            && tab.is_dirty != dirty
        {
            tab.is_dirty = dirty;
        }
    });

    // ── Save As ──────────────────────────────────────────────────────────────
    //
    // Picks a destination via the platform save dialog, exports DOCX to it, then
    // repoints the current tab at the new file and records it in recents. This
    // is the only way to persist an untitled document.
    let doc_state_saveas = Arc::clone(&doc_state);
    let save_as = use_callback(move |_: ()| {
        let doc_state = Arc::clone(&doc_state_saveas);
        let mut tabs = tabs;
        let mut recent = recent_docs;
        let mut save_message = save_message;
        let mut baseline_gen = baseline_gen;
        let nav = navigator;
        let cur_path = path_signal.peek().clone();
        let suggested = {
            let stem = display_title_from_path(&cur_path);
            format!("{stem}.docx")
        };
        spawn(async move {
            let picker = FilePicker::new();
            let opts = SaveOptions {
                mime_type: Some(DOCX_MIME.to_string()),
                suggested_name: Some(suggested),
            };
            match picker.pick_file_to_save(opts).await {
                Ok(Some(token)) => match export_document_to_token(&token, &doc_state) {
                    Ok(()) => {
                        let new_path = token.serialize();
                        let new_title = display_title_from_path(&new_path);
                        {
                            let mut t = tabs.write();
                            if let Some(tab) = t.iter_mut().find(|tab| tab.path == cur_path) {
                                tab.path = new_path.clone();
                                tab.title = new_title.clone();
                                tab.is_dirty = false;
                            }
                        }
                        recent.write().record(new_path.clone(), new_title);
                        recent.read().save();
                        save_message.set(Some(fl!("editor-save-success")));
                        // Navigate to the saved file; the editor reloads it and
                        // re-establishes a clean baseline.
                        baseline_gen.set(0);
                        nav.push(Route::Editor { path: new_path });
                    }
                    Err(e) => {
                        save_message.set(Some(fl!("editor-save-error", reason = e.to_string())));
                    }
                },
                Ok(None) => { /* user cancelled — no-op */ }
                Err(e) => {
                    save_message.set(Some(fl!("editor-save-error", reason = e.to_string())));
                }
            }
        });
    });

    // ── Ctrl+S handler ───────────────────────────────────────────────────────
    //
    // The keydown handler bumps `save_request`; perform the save here, where the
    // tab/recents context is reachable. Untitled documents route to Save As.
    let doc_state_savereq = Arc::clone(&doc_state);
    use_effect(move || {
        let n = save_request(); // subscribe — fires on each Ctrl+S
        if n == 0 {
            return; // initial value — nothing requested yet
        }
        let path = path_signal.peek().clone();
        if is_untitled(&path) {
            save_as.call(());
            return;
        }
        let msg = match save_document_to_path(&path, &doc_state_savereq) {
            Ok(()) => {
                baseline_gen.set(cursor_state.peek().document_generation);
                fl!("editor-save-success")
            }
            Err(e) => fl!("editor-save-error", reason = e.to_string()),
        };
        save_message.set(Some(msg));
    });

    // ── Measure the canvas at mount ──────────────────────────────────────────
    //
    // Scroll metrics are otherwise only populated by the first DOM scroll
    // event, which would leave the view-mode default and the reflow layout
    // width unknown until the user scrolls. Query the mounted scroll
    // container's client rect once (backed by the dioxus-native MountedData
    // patch) and seed the metrics.
    use_effect(move || {
        let Some(evt) = canvas_mounted() else { return };
        if scroll_metrics.peek().client_width > 0.0 {
            return;
        }
        let mut metrics = scroll_metrics;
        spawn(async move {
            if let Ok(rect) = evt.get_client_rect().await {
                let mut m = metrics.write();
                if m.client_width <= 0.0 {
                    m.client_width = rect.size.width as f32;
                    m.client_height = rect.size.height as f32;
                }
            }
        });
    });

    // ── Default view mode by viewport width ──────────────────────────────────
    //
    // Paginated on wide viewports, reflowed on narrow ones — until the user
    // picks a mode explicitly (which sets `view_mode_user_set` and freezes this
    // default). The viewport width becomes known from the scroll container's
    // `client_width`, reported on the first DOM scroll event.
    use_effect(move || {
        if *view_mode_user_set.read() {
            return;
        }
        let width = scroll_metrics.read().client_width;
        if width <= 0.0 {
            return;
        }
        let desired = if width < REFLOW_BREAKPOINT_PX {
            ViewMode::Reflow
        } else {
            ViewMode::Paginated
        };
        if *view_mode.peek() != desired {
            view_mode.set(desired);
        }
    });

    let canvas_hovered = use_signal(|| false);
    let page_gap_px = tokens::PAGE_GAP_PX;

    let page_label = if view_mode() == ViewMode::Reflow {
        // Reflow has no fixed pages — hide the page indicator entirely.
        String::new()
    } else if total_pages() == 0 {
        fl!("editor-page-loading") // empty in en-US — avoids flash while loading
    } else {
        fl!(
            "editor-page-label",
            current = current_page() as i64,
            total = total_pages() as i64
        )
    };

    let font_substitutions = {
        if let Ok(state) = doc_state.lock() {
            if let Ok(fr) = state.shared_font_resources.lock() {
                fr.substitutions.clone()
            } else {
                std::collections::HashMap::new()
            }
        } else {
            std::collections::HashMap::new()
        }
    };

    let mut substituted_items = Vec::new();
    let mut missing_items = Vec::new();
    let mut download_links = Vec::new();

    for (requested, sub) in &font_substitutions {
        if let Some(sub_name) = sub {
            substituted_items.push(format!("{} → {}", requested, sub_name));
        } else {
            missing_items.push(requested.clone());
        }

        let link = match requested.to_lowercase().as_str() {
            "aptos" => Some((
                "Aptos",
                "https://www.microsoft.com/en-us/download/details.aspx?id=106037",
            )),
            "calibri" => Some((
                "Calibri",
                "https://learn.microsoft.com/en-us/typography/font-list/calibri",
            )),
            "cambria" => Some((
                "Cambria",
                "https://learn.microsoft.com/en-us/typography/font-list/cambria",
            )),
            "arial" => Some((
                "Arial",
                "https://learn.microsoft.com/en-us/typography/font-list/arial",
            )),
            "courier new" => Some((
                "Courier New",
                "https://learn.microsoft.com/en-us/typography/font-list/courier-new",
            )),
            "times new roman" => Some((
                "Times New Roman",
                "https://learn.microsoft.com/en-us/typography/font-list/times-new-roman",
            )),
            _ => None,
        };
        if let Some((label, url)) = link {
            download_links.push((label, url));
        }
    }

    download_links.sort_by_key(|(lbl, _)| *lbl);
    download_links.dedup_by_key(|(lbl, _)| *lbl);

    let sub_text = if !substituted_items.is_empty() {
        format!("Substituted: {}. ", substituted_items.join(", "))
    } else {
        String::new()
    };

    let miss_text = if !missing_items.is_empty() {
        format!("Missing: {}. ", missing_items.join(", "))
    } else {
        String::new()
    };

    let font_warning_details = format!("{}{}", sub_text, miss_text);

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; flex: 1; \
                 overflow: hidden; background: {bg}; font-family: {ff};",
                bg = tokens::COLOR_SURFACE_BASE,
                ff = tokens::FONT_FAMILY_UI,
            ),

            // ── Scrollable page canvas ────────────────────────────────────────
            {render_canvas_area(
                doc_state_mousedown,
                doc_state_mousemove,
                doc_state_touch,
                doc_state_touchend,
                doc_state_keydown,
                doc_state_render,
                doc_state_scroll,
                is_dragging,
                drag_origin,
                touch_state,
                window_width,
                scroll_offset,
                scroll_metrics,
                canvas_mounted,
                vbar_drag,
                hbar_drag,
                current_page,
                total_pages,
                view_mode,
                cursor_state,
                loro_doc,
                undo_manager,
                can_undo,
                can_redo,
                save_request,
                path_signal,
                document_load,
                canvas_hovered,
                page_gap_px,
            )}

            // ── Font Warning Banner ──────────────────────────────────────────
            if !font_substitutions.is_empty() && !dismiss_font_warning() {
                div {
                    style: format!(
                        "display: flex; flex-direction: row; align-items: center; justify-content: space-between; \
                         padding: {p}px {p2}px; background: {bg}; border-top: 1px solid {border}; \
                         border-bottom: 1px solid {border}; font-family: {ff}; font-size: {size}px; \
                         color: {fg}; flex-shrink: 0;",
                        p      = tokens::SPACE_2,
                        p2     = tokens::SPACE_4,
                        bg     = tokens::COLOR_SURFACE_2,
                        border = tokens::COLOR_CONTEXTUAL_TAB,
                        ff     = tokens::FONT_FAMILY_UI,
                        size   = tokens::FONT_SIZE_BODY - 1.0,
                        fg     = tokens::COLOR_TEXT_ON_CHROME,
                    ),
                    div {
                        style: "display: flex; flex-direction: column; gap: 4px; flex: 1;",
                        div {
                            style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
                            span {
                                style: format!("color: {}; font-weight: bold;", tokens::COLOR_CONTEXTUAL_TAB),
                                "⚠️ {fl!(\"editor-font-substitution-title\")}:"
                            }
                            span { {fl!("editor-font-substitution-message")} }
                        }
                        span {
                            style: format!("font-size: {size}px; color: {fg_sec};",
                                size   = tokens::FONT_SIZE_LABEL,
                                fg_sec = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                            ),
                            "{font_warning_details}"
                        }
                    }
                    div {
                        style: "display: flex; flex-direction: row; align-items: center; gap: 16px; margin-left: 16px;",
                        if !download_links.is_empty() {
                            div {
                                style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
                                span {
                                    style: format!("font-size: {size}px; color: {fg_sec};",
                                        size   = tokens::FONT_SIZE_LABEL,
                                        fg_sec = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                                    ),
                                    {fl!("editor-font-substitution-download")}
                                }
                                {
                                    download_links.iter().map(|(label, url)| rsx! {
                                        a {
                                            key: "{label}",
                                            style: format!(
                                                "color: {accent}; text-decoration: underline; font-size: {size}px; cursor: pointer;",
                                                accent = tokens::COLOR_TAB_ACTIVE_INDICATOR,
                                                size   = tokens::FONT_SIZE_LABEL,
                                            ),
                                            href: "{url}",
                                            target: "_blank",
                                            "{label}"
                                        }
                                    })
                                }
                            }
                        }
                        button {
                            style: format!(
                                "padding: {p}px {p2}px; background: {bg}; border: 1px solid {border}; \
                                 border-radius: 4px; color: {fg}; font-size: {size}px; cursor: pointer; \
                                 margin-left: 8px;",
                                p      = tokens::SPACE_1,
                                p2     = tokens::SPACE_2,
                                bg     = tokens::COLOR_SURFACE_3,
                                border = tokens::COLOR_BORDER_CHROME,
                                fg     = tokens::COLOR_TEXT_ON_CHROME,
                                size   = tokens::FONT_SIZE_LABEL,
                            ),
                            onclick: move |_| {
                                dismiss_font_warning.set(true);
                            },
                            {fl!("editor-font-dismiss")}
                        }
                    }
                }
            }

            // ── Paragraph style picker panel (inline, above ribbon) ───────────
            // Rendered between canvas and ribbon in the flex column so it
            // works without position: absolute (unsupported in Blitz).
            // COMPAT(dioxus-native): see editor_style.rs for layout rationale.
            if *is_style_picker_open.read() {
                {style_picker_panel(
                    doc_state_style_picker,
                    loro_doc,
                    cursor_state,
                    undo_manager,
                    can_undo,
                    can_redo,
                    current_style_name.clone(),
                    is_style_picker_open,
                    style_search_query,
                )}
            }

            // ── Style catalog editor panel (inline, above ribbon) ─────────────
            // COMPAT(dioxus-native): position: absolute is unsupported in
            // Blitz — rendered inline in the flex column, above the ribbon.
            if editing_style_draft.read().is_some() {
                {style_editor_panel(
                    doc_state_style_editor,
                    loro_doc,
                    editing_style_draft,
                )}
            }

            // ── Metadata editor panel (Dublin Core) ───────────────────────────
            if editing_metadata.read().is_some() {
                {metadata_panel(
                    doc_state_meta,
                    editing_metadata,
                    save_message,
                    super::editor_metadata_panel::MetaPanelSync {
                        loro_doc,
                        cursor_state,
                        undo_manager,
                        can_undo,
                        can_redo,
                    },
                )}
            }

            // ── PDF/X export panel (conformance-level picker) ─────────────────
            if is_publish_panel_open() {
                {publish_panel(
                    doc_state_publish_panel,
                    path_signal,
                    save_message,
                    is_publish_panel_open,
                    pdf_level,
                )}
            }

            // ── Save message banner ───────────────────────────────────────────
            if let Some(msg) = save_message.read().clone() {
                div {
                    style: format!(
                        "display: flex; flex-direction: row; align-items: center; \
                         justify-content: space-between; padding: {p}px {p2}px; \
                         background: {bg}; border-top: 1px solid {border}; \
                         font-family: {ff}; font-size: {size}px; \
                         color: {fg}; flex-shrink: 0;",
                        p      = tokens::SPACE_2,
                        p2     = tokens::SPACE_4,
                        bg     = tokens::COLOR_SURFACE_2,
                        border = tokens::COLOR_BORDER_CHROME,
                        ff     = tokens::FONT_FAMILY_UI,
                        size   = tokens::FONT_SIZE_LABEL,
                        fg     = tokens::COLOR_TEXT_ON_CHROME,
                    ),
                    span { "{msg}" }
                    button {
                        style: format!(
                            "background: transparent; border: none; font-size: {fs}px; \
                             color: {fg}; cursor: pointer; padding: {p}px;",
                            fs = tokens::FONT_SIZE_LABEL,
                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                            p  = tokens::SPACE_1,
                        ),
                        onclick: move |_| { save_message.set(None); },
                        "\u{2715}"
                    }
                }
            }

            // ── Ribbon (formatting controls) ──────────────────────────────────
            AtRibbon {
                tabs: vec![
                    RibbonTabDesc { label: fl!("ribbon-tab-home"),    is_contextual: false, aria_label: None },
                    RibbonTabDesc { label: fl!("ribbon-tab-insert"),  is_contextual: false, aria_label: None },
                    RibbonTabDesc { label: fl!("ribbon-tab-format"),  is_contextual: false, aria_label: None },
                    RibbonTabDesc { label: fl!("ribbon-tab-review"),  is_contextual: false, aria_label: None },
                    RibbonTabDesc { label: fl!("ribbon-tab-view"),    is_contextual: false, aria_label: None },
                    RibbonTabDesc { label: fl!("ribbon-tab-publish"), is_contextual: false, aria_label: None },
                ],
                active_tab: active_ribbon_tab(),
                on_tab_select: move |idx| {
                    active_ribbon_tab.set(idx);
                },
                collapsed: ribbon_collapsed(),
                on_toggle_collapse: move |_| {
                    ribbon_collapsed.set(!ribbon_collapsed());
                },
                toggle_aria_label: if ribbon_collapsed() {
                    fl!("ribbon-expand-aria")
                } else {
                    fl!("ribbon-collapse-aria")
                },
                tab_content: match active_ribbon_tab() {
                    5 => publish_tab_content(
                        &doc_state_publish,
                        path_signal,
                        save_message,
                        is_publish_panel_open,
                        editing_metadata,
                    ),
                    _ => home_tab_content(
                    &doc_state_ribbon,
                    loro_doc,
                    cursor_state,
                    undo_manager,
                    can_undo,
                    can_redo,
                    bold_active,
                    italic_active,
                    underline_active,
                    strikethrough_active,
                    superscript_active,
                    subscript_active,
                    current_style_name,
                    is_style_picker_open,
                    path_signal,
                    save_message,
                    editing_style_draft,
                    save_as,
                    baseline_gen,
                ),
                },
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
                view_mode_label:    if view_mode() == ViewMode::Reflow {
                    fl!("editor-view-reflowed")
                } else {
                    fl!("editor-view-paginated")
                },
                view_mode_aria_label: fl!("editor-view-toggle-aria"),
                on_view_mode_click: move |_| {
                    // User override freezes the width-based default.
                    view_mode_user_set.set(true);
                    let next = if *view_mode.peek() == ViewMode::Reflow {
                        ViewMode::Paginated
                    } else {
                        ViewMode::Reflow
                    };
                    view_mode.set(next);
                },
            }
        }
    }
}
