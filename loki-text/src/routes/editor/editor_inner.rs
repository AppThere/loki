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

use appthere_ui::{AtRibbon, AtStatusBar, tokens, use_breakpoint};
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
use super::editor_docked_panels::{DockedSync, docked_panels};
use super::editor_load::load_document;
use super::editor_metadata_panel::metadata_panel;
use super::editor_path_sync::{
    PathSyncSignals, restore_session, stash_outgoing, sync_path_and_reset,
};
use super::editor_publish::publish_panel;
use super::editor_ribbon::write_tab_content;
use super::editor_ribbon_insert::insert_tab_content;
use super::editor_ribbon_publish::publish_tab_content;
use super::editor_save_banner::save_banner;
use super::editor_spell::SpellMenu;
use super::editor_state::{EditorState, StyleDraft, use_editor_state};
use super::editor_style::style_picker_panel;
use super::editor_style_editor::style_editor_panel;
use crate::error::LoadError;
use crate::sessions::DocSessions;
use crate::tabs::OpenTab;
use loki_app_shell::spell::SpellService;

// EditorMode removed — always edit mode; distraction-free reading is the View tab (future pass).

/// Document editor inner component — all editing logic lives here.
///
/// Document switching is handled reactively via `path_signal` — see the
/// module-level doc for the full design.
#[component]
pub(super) fn EditorInner(path: String) -> Element {
    // ── Path signal: bridge from prop-space to signal-space ──────────────────
    let mut path_signal: Signal<String> = use_signal(|| path.clone());

    // ── Font-substitution detail panel open state ────────────────────────────
    // Closed by default; the status-bar chip (shown whenever substitutions
    // exist) toggles it.
    let mut font_panel_open = use_signal(|| false);

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
        mut saved_state,
        can_undo,
        can_redo,
        is_style_picker_open,
        editing_style_draft,
        mut zoom_percent,
        is_dirty,
        save_message,
        save_request,
        mut active_ribbon_tab,
        open_color_picker,
        recent_text_colors,
        recent_highlights,
        is_publish_panel_open,
        pdf_level,
        editing_metadata,
    } = use_editor_state();

    // ── Tab/recents context for Save As and the unsaved-changes indicator ────
    let tabs = use_context::<Signal<Vec<OpenTab>>>();
    // Spell-check service (app-root context): right-click suggestions panel + language picker.
    let spell_service = use_context::<SpellService>();
    let spell_menu = use_signal(|| Option::<SpellMenu>::None);
    let is_language_panel_open = use_signal(|| false);
    let language_status = use_signal(|| Option::<String>::None);
    // Key of the spelling-menu row currently hovered (Blitz has no CSS :hover).
    let spell_hover = use_signal(|| Option::<String>::None);
    // Insert-tab hyperlink panel: `Some(url)` while open (Spec 04 M4).
    let link_draft = use_signal(|| Option::<String>::None);
    // Character style in the style panel (Spec 05 M6): id → inspector, draft → form.
    let editing_char_style = use_signal(|| Option::<String>::None);
    let editing_char_draft = use_signal(|| Option::<StyleDraft>::None);
    let editing_table_style = use_signal(|| Option::<String>::None);
    let editing_table_draft = use_signal(super::editor_style_editor::table_draft_none);
    // List / page styles browsed in the style panel (Spec 05 M6): read-only.
    let editing_list_style = use_signal(|| Option::<String>::None);
    let editing_page_style = use_signal(|| Option::<String>::None);
    // Compact style-panel pane (Spec 05 M7 §11): Inspect vs Edit; ignored ≥Medium.
    let style_panel_inspect = use_signal(|| false);
    // Stashed sessions for inactive tabs — unsaved edits survive tab switches.
    let doc_sessions = use_context::<Signal<DocSessions>>();
    // "Clean" generation (matches disk), captured at load/save; tab is dirty when live gen differs.
    let mut baseline_gen = use_signal(|| 0_u64);

    // The per-document signals reset or restored on tab switch, bundled for the
    // three handover sites below (every field is a `Copy` signal).
    let path_sync_signals = move || PathSyncSignals {
        cursor_state,
        loro_doc,
        undo_manager,
        total_pages,
        current_page,
        can_undo,
        can_redo,
        font_panel_open,
        is_style_picker_open,
        open_color_picker,
        editing_style_draft,
        save_message,
        baseline_gen,
        saved_state,
    };

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
                let mut sig = path_sync_signals();
                restore_session(session, &doc_state_restore, &mut sig, path_signal);
            }
        });
    }

    // ── Session stash at unmount ─────────────────────────────────────────────
    //
    // `stash_outgoing` itself skips the stash when no tab still points at the
    // path (the tab was closed, and Shell already dropped the session — re-
    // stashing would resurrect discarded edits on reopen).
    {
        let doc_state_drop = Arc::clone(&doc_state);
        let tabs_at_drop = tabs;
        let mut sessions_at_drop = doc_sessions;
        use_drop(move || {
            let path = path_signal.peek().clone();
            let mut sig = path_sync_signals();
            stash_outgoing(
                &path,
                &doc_state_drop,
                tabs_at_drop,
                &mut sessions_at_drop,
                &mut sig,
            );
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
        tabs,
        doc_sessions,
        &mut path_sync_signals(),
    );

    // Current paragraph style name, from signals — updates in the same render cycle as the cursor.
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
    let doc_state_docked = Arc::clone(&doc_state);
    let doc_state_style_picker = Arc::clone(&doc_state);
    let doc_state_style_editor = Arc::clone(&doc_state);
    let doc_state_spell_ctx = Arc::clone(&doc_state);
    let doc_state_seed = Arc::clone(&doc_state);
    let doc_state_render = Arc::clone(&doc_state);
    let doc_state_scroll = Arc::clone(&doc_state);

    // Font-family enumeration for the style editor's picker — async so the
    // mount never blocks on the background system-font warm-up.
    let font_families = super::editor_fonts::use_font_families(&doc_state);

    // ── Document load — reactive on path_signal ───────────────────────────────
    let document_load: Resource<(String, Result<Document, LoadError>)> = use_resource(move || {
        let p = path_signal();
        async move {
            let res = load_document(p.clone());
            (p, res)
        }
    });

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
            tracing::info!(
                target: "loki_text::open",
                "open: loro effect firing — cloning document + spawning layout task",
            );
            let loaded_path = loaded_path.clone();
            let clone_start = std::time::Instant::now();
            let doc = doc.clone();
            tracing::info!(
                target: "loki_text::open",
                clone_ms = clone_start.elapsed().as_secs_f64() * 1000.0,
                "open: document cloned",
            );
            let doc_state_seed = Arc::clone(&doc_state_seed);
            spawn(async move {
                let open_start = std::time::Instant::now();
                tracing::info!(target: "loki_text::open", "open: layout task polled (start)");
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
                        let mut um = loro::UndoManager::new(&l_doc);
                        // Pair a fresh clean-checkpoint tracker with the fresh
                        // undo manager (depth 0 = the on-disk state).
                        let tracker = crate::editing::saved_state::SavedStateHandle::new();
                        tracker.attach(&mut um);
                        saved_state.set(tracker);
                        loro_doc.set(Some(l_doc));
                        undo_manager.set(Some(um));

                        // The freshly-loaded document matches the file on disk:
                        // record the current generation as the clean baseline.
                        baseline_gen.set(cursor_state.peek().document_generation);

                        // Auto-place the cursor at the start of the document so
                        // the user can type immediately without clicking first.
                        if cursor_state.read().focus.is_none() {
                            use crate::editing::cursor::DocumentPosition;
                            let start = DocumentPosition::top_level(0, 0, 0);
                            let mut cs = cursor_state.write();
                            cs.anchor = Some(start.clone());
                            cs.focus = Some(start);
                        }

                        // Lifting the canvas's loading gate (total_pages > 0)
                        // mounts the GPU DocumentView, whose first paint blocks
                        // the main thread. Defer it one scheduler tick so the
                        // loading indicator paints a frame first — that frame
                        // then stays on screen through the GPU first-paint freeze
                        // instead of a blank canvas.
                        spawn(async move {
                            total_pages.set(page_count as u32);
                            tracing::info!(
                                target: "loki_text::open",
                                pages = page_count,
                                elapsed_ms = open_start.elapsed().as_secs_f64() * 1000.0,
                                "open: layout ready, DocumentView mounting (CPU done)",
                            );
                        });
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

    // Live status-bar word count, recomputed per mutation (F7c / 4c.5).
    let word_count_label =
        crate::editing::word_count::use_word_count_label(Arc::clone(&doc_state), cursor_state);

    // Unsaved-changes (dirty) tracking → tab indicator + ribbon Save state.
    // Also clears a lingering success chip the moment the document goes dirty.
    super::editor_dirty::use_dirty_tracking(
        cursor_state,
        path_signal,
        baseline_gen,
        saved_state,
        is_dirty,
        tabs,
        save_message,
    );
    // Success statuses ("Document saved", …) clear themselves after a moment.
    super::editor_save_banner::use_save_status_autoclear(save_message);

    // ── Save As / Save as Template (extracted flows: editor_save_callbacks) ──
    let save_as = super::editor_save_callbacks::use_save_as_callback(
        Arc::clone(&doc_state),
        save_message,
        baseline_gen,
        path_signal,
    );
    let save_as_template = super::editor_save_callbacks::use_save_as_template_callback(
        Arc::clone(&doc_state),
        save_message,
        path_signal,
    );

    // ── Insert tab handles (image insertion at the cursor) ────────────────────
    let insert_ctx = super::editor_ribbon_insert::InsertCtx {
        doc_state: Arc::clone(&doc_state),
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
        save_message,
    };

    // ── Ctrl+S handler (extracted flow — see editor_save_callbacks) ─────────
    super::editor_save_callbacks::use_ctrl_s_save(super::editor_save_callbacks::CtrlSCtx {
        doc_state: Arc::clone(&doc_state),
        path_signal,
        save_request,
        save_as,
        baseline_gen,
        cursor_state,
        loro_doc,
        undo_manager,
        saved_state,
        can_undo,
        can_redo,
        save_message,
    });

    // ── Viewport-driven effects (Spec 03 M1/M2) ──────────────────────────────
    // Seed metrics at mount, pick the renderer by zoom-aware page-fit, publish the
    // measured width + live zoom to the responsive context. See `editor_responsive`.
    super::editor_responsive::use_viewport_effects(
        canvas_mounted,
        scroll_metrics,
        std::sync::Arc::clone(&doc_state),
        view_mode,
        view_mode_user_set,
        zoom_percent,
    );

    // Contextual ribbon tabs (Spec 04 M5 / plan 4a.2): a Table tab appears while the caret is in a table.
    let (ribbon_tabs, table_selected) =
        super::editor_ribbon_table::use_ribbon_tabs(cursor_state, active_ribbon_tab);

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

    // Font substitutions reported by the layout engine (requested → substitute):
    // the status-bar chip is the indicator; the detail panel opens from it.
    let font_substitutions = super::editor_fonts::font_substitutions(&doc_state);
    let font_sub_count = font_substitutions.len() as i64;

    rsx! {
        div {
            style: format!(
                // `position: relative` establishes the containing block for the
                // floating spelling menu (an absolutely-positioned child).
                "display: flex; flex-direction: column; flex: 1; position: relative; \
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
                spell_service.clone(),
                spell_menu,
                doc_state_spell_ctx,
                zoom_percent,
            )}

            // ── Font-substitution detail panel (Spec 03 M3, inverted) ─────────
            // The indicator is the status-bar chip below; this panel opens on
            // demand from that chip. Breakpoint-aware (table vs. card stack);
            // renders nothing while closed or when there are no substitutions.
            super::editor_font_warning::FontSubstitutionPanel {
                substitutions: font_substitutions.clone(),
                open: font_panel_open,
            }

            // ── Colour-picker panel (inline, above ribbon) ────────────────────
            // Opened by the Format tab's Font colour / Highlight triggers.
            if let Some(target) = open_color_picker() {
                {super::editor_color_panel::color_picker_panel(
                    &doc_state_ribbon,
                    target,
                    open_color_picker,
                    super::editor_ribbon_format::RibbonEditCtx {
                        loro_doc, cursor_state, undo_manager, can_undo, can_redo,
                    },
                    recent_text_colors,
                    recent_highlights,
                )}
            }

            // ── Paragraph style picker panel (inline, above ribbon) ───────────
            // Rendered between canvas and ribbon in the flex column — an in-flow
            // choice (block-level position: absolute is now confirmed in Blitz;
            // see editor_style.rs for the layout rationale).
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
            // Rendered inline in the flex column, above the ribbon (an in-flow
            // choice; block-level position: absolute is now confirmed in Blitz).
            if editing_style_draft.read().is_some() {
                {style_editor_panel(
                    doc_state_style_editor,
                    editing_style_draft,
                    editing_char_style,
                    editing_char_draft,
                    editing_table_style,
                    editing_table_draft,
                    editing_list_style,
                    editing_page_style,
                    style_panel_inspect,
                    use_breakpoint(),
                    font_families(),
                    super::editor_style_editor::StyleEditorSync {
                        loro_doc,
                        cursor_state,
                        undo_manager,
                        can_undo,
                        can_redo,
                        save_message,
                    },
                )}
            }

            // ── Docked panels: spelling menu, language picker, Insert link ────
            // Each self-gates on its trigger signal. Docked above the ribbon
            // in flow; the spelling menu uses position: absolute (confirmed).
            {docked_panels(
                doc_state_docked,
                DockedSync {
                    loro_doc,
                    cursor_state,
                    undo_manager,
                    can_undo,
                    can_redo,
                },
                spell_service.clone(),
                spell_menu,
                is_language_panel_open,
                language_status,
                spell_hover,
                scroll_metrics().client_width,
                link_draft,
            )}

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

            // ── Save/export error banner (successes are the status chip) ─────
            {save_banner(save_message)}

            // ── Ribbon (formatting controls) ──────────────────────────────────
            AtRibbon {
                // Core tabs + a Table contextual tab (appended by `use_ribbon_tabs` in a table).
                tabs: ribbon_tabs,
                active_tab: active_ribbon_tab(),
                on_tab_select: move |idx| active_ribbon_tab.set(idx),
                collapsed: ribbon_collapsed(),
                on_toggle_collapse: move |_| ribbon_collapsed.set(!ribbon_collapsed()),
                toggle_aria_label: if ribbon_collapsed() {
                    fl!("ribbon-expand-aria")
                } else {
                    fl!("ribbon-collapse-aria")
                },
                tab_content: match active_ribbon_tab() {
                    1 => super::editor_ribbon_span::format_tab_content(
                        loro_doc, cursor_state, open_color_picker,
                    ),
                    2 => insert_tab_content(link_draft, insert_ctx.clone()),
                    7 if table_selected => super::editor_ribbon_table::table_tab_content(
                        &doc_state_ribbon, loro_doc, cursor_state, undo_manager, can_undo, can_redo,
                    ),
                    3 => super::editor_ribbon_layout::layout_tab_content(&doc_state_ribbon, loro_doc, cursor_state, undo_manager, can_undo, can_redo),
                    4 => super::editor_ribbon_references::references_tab_content(&doc_state_ribbon, loro_doc, cursor_state, undo_manager, can_undo, can_redo),
                    5 => super::editor_ribbon_review::review_tab_content(&doc_state_ribbon, loro_doc, cursor_state, undo_manager, can_undo, can_redo),
                    6 => publish_tab_content(
                        &doc_state_publish, path_signal, save_message,
                        is_publish_panel_open, editing_metadata,
                    ),
                    _ => write_tab_content(
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
                    save_request,
                    is_dirty,
                    editing_style_draft,
                    save_as,
                    save_as_template,
                ),
                },
            }

            // ── Status bar ────────────────────────────────────────────────────
            AtStatusBar {
                page_label:         page_label,
                word_count_label:   word_count_label(),
                language_label:     fl!("editor-language"),
                zoom_percent:       zoom_percent(),
                collaborator_count: 0,
                collaborator_label: String::new(),
                zoom_aria_label:    fl!("editor-zoom-aria"),
                on_zoom_click:      move |_| {
                    let next = appthere_ui::next_zoom(*zoom_percent.peek());
                    zoom_percent.set(next);
                },
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
                // Font-substitution indicator (Spec 03 M3, inverted): the chip
                // is the always-on signal that fonts were substituted; clicking
                // it toggles the detail panel above the ribbon.
                notice_label: if font_sub_count > 0 {
                    fl!("editor-font-substitution-chip", count = font_sub_count)
                } else {
                    String::new()
                },
                notice_aria_label: fl!("editor-font-substitution-title"),
                on_notice_click:    move |_| {
                    let v = *font_panel_open.peek();
                    font_panel_open.set(!v);
                },
                // Transient success chip ("Document saved", …). Auto-clears
                // (use_save_status_autoclear) and clears on dirty; click = dismiss.
                status_note_label: super::editor_save_banner::save_status_chip_label(save_message),
                on_status_note_click: {
                    let mut save_message = save_message;
                    move |_| save_message.set(None)
                },
            }
        }
    }
}
