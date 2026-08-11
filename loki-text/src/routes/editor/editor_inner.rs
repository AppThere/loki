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

use appthere_ui::{AtRibbon, tokens, use_breakpoint, use_device_profile, use_viewport_controller};
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
use super::editor_path_sync::{
    PathSyncSignals, restore_session, stash_outgoing, sync_path_and_reset,
};
use super::editor_publish::publish_panel;
use super::editor_ribbon::write_tab_content;
use super::editor_ribbon_insert::insert_tab_content;
use super::editor_ribbon_publish::publish_tab_content;
use super::editor_save_banner::save_banner;
use super::editor_seed_publish::{SeedTargets, publish_seed_and_mirror};
use super::editor_spell::SpellMenu;
use super::editor_state::{EditorState, StyleDraft, use_editor_state};
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
    let font_panel_open = use_signal(|| false);
    // Raised by Actual Size on a display whose density is not yet known.
    let calibrating = use_signal(|| false);

    // ── Ribbon collapse state ────────────────────────────────────────────────
    let mut ribbon_collapsed = use_signal(|| false);

    // ── Style search query (cleared on picker close) ─────────────────────────
    let style_search_query = use_signal(String::new);
    // Bumped when an app-scoped setting is written (T6.3/T6.4). Those live in a
    // file rather than in reactive state, so the style panel — which re-reads
    // them each render — needs something to re-render on.
    let settings_generation = use_signal(|| 0u64);

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
        view_mode,
        view_mode_user_set,
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
        is_char_style_picker_open,
        editing_style_draft,
        zoom_percent,
        is_dirty,
        save_message,
        save_request,
        mut active_ribbon_tab,
        open_color_picker,
        recent_text_colors,
        recent_highlights,
        is_publish_panel_open,
        pdf_level,
        paragraph_style_dialog,
        dialogs,
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
    // Character style in the style panel (Spec 05 M6): id → inspector, draft → form.
    let editing_char_style = use_signal(|| Option::<String>::None);
    let editing_char_draft = use_signal(|| Option::<StyleDraft>::None);
    let editing_table_style = use_signal(|| Option::<String>::None);
    let editing_table_draft = use_signal(super::editor_style_editor::table_draft_none);
    // List / page styles browsed in the style panel (Spec 05 M6): read-only.
    let editing_list_style = use_signal(|| Option::<String>::None);
    let editing_list_level = use_signal(super::editor_style_editor::list_level_draft_none);
    let editing_page_style = use_signal(|| Option::<String>::None);
    // Compact style-panel pane (Spec 05 M7 §11): Inspect vs Edit; ignored ≥Medium.
    let style_panel_inspect = use_signal(|| false);
    // Proc name requested by a MACROBUTTON click, consumed by MacroNoticeBar (§6).
    let macro_run_request = use_signal(|| Option::<String>::None);
    // Stashed sessions for inactive tabs — unsaved edits survive tab switches.
    let doc_sessions = use_context::<Signal<DocSessions>>();
    // "Clean" generation (matches disk), captured at load/save; tab is dirty when live gen differs.
    let baseline_gen = use_signal(|| 0_u64);

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
    let doc_state_publish_panel = Arc::clone(&doc_state);
    let doc_state_docked = Arc::clone(&doc_state);
    let doc_state_style_picker = Arc::clone(&doc_state);
    let doc_state_style_editor = Arc::clone(&doc_state);
    let doc_state_modals = Arc::clone(&doc_state);
    let doc_state_zoom = Arc::clone(&doc_state);
    let doc_state_cap = Arc::clone(&doc_state);
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
        async move { (p.clone(), load_document(p)) }
    });

    // Repair banner: self-contained hook (detection + state + mount).
    let repair_banner = super::editor_repair_banner::use_repair_banner(path_signal, save_message);
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

                // Seed, mirror (I-10) and clean baseline together — see its docs.
                let targets = SeedTargets {
                    cursor_state,
                    baseline_gen,
                };
                let page_count = publish_seed_and_mirror(&doc_state_seed, &doc, layout, targets);

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

    // ── Page count sync — re-runs on load and on every mutation ──────────────
    //
    // `doc_state.page_count` lives behind a Mutex, which nothing can subscribe
    // to, so `total_pages` is its reactive mirror — the I-10 pattern
    // (`editor_seed_publish`). Subscribing only to the load resource left the
    // edit path unmirrored: typing onto a new page showed "Page 2 of 1" until
    // a tab switch. The generation memo (bumped by `post_mutation_sync` after
    // every relayout) covers growth *and* shrinkage while typing.
    let page_sync_generation = use_memo(move || cursor_state.read().document_generation);
    use_effect(move || {
        // Reactive reads — subscribe to the load resource (open path) and the
        // mirrored document generation (edit path).
        let resource_signal = document_load.value();
        let _sub = resource_signal.read();
        let _gen = page_sync_generation();
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
    // The Document group's New/Open/Save-a-Copy callbacks + recents handle (§4).
    let (document_actions, ribbon_recents) = super::editor_document_actions::use_document_group(
        &doc_state,
        save_message,
        path_signal,
        save_as,
        save_as_template,
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

    // ── Zoom measurements (Spec 08 T5.4 / T5.5) ───────────────────────────
    // Read here because this is where the layout state and the measured canvas
    // both are; each returns `None` until its input is real, so a fit or a cap
    // is never computed from a placeholder. See `editor_zoom`.
    let zoom_fit_inputs = move || super::editor_zoom::fit_inputs(&doc_state_zoom, scroll_metrics());
    let display_ppi = move || use_device_profile().display.and_then(|d| d.css_px_per_inch);
    let zoom_cap_permille = move || super::editor_zoom::capability_permille(&doc_state_cap);
    // The one way the zoom changes, so anchoring cannot be forgotten at one of
    // the six call sites (Spec 08 T5.6).
    let zoom_command = super::editor_zoom::ZoomCommand::new(
        zoom_percent,
        use_viewport_controller(scroll_metrics, canvas_mounted),
    );

    // Font substitutions reported by the layout engine (requested → substitute):
    // the status-bar chip is the indicator; the detail panel opens from it.
    let font_substitutions = super::editor_fonts::font_substitutions(&doc_state);
    let font_sub_count = font_substitutions.len() as i64;

    // Built once: the inline style panel and the paragraph style dialog write
    // through the same handles, and two copies of this literal is two chances
    // for them to disagree about which signals a style edit updates.
    let style_sync = super::editor_style_editor::StyleEditorSync {
        loro_doc,
        cursor_state,
        undo_manager,
        can_undo,
        can_redo,
        save_message,
        settings_generation,
    };
    let style_panel_state = super::editor_style_panels::StylePanelState {
        is_style_picker_open,
        is_char_style_picker_open,
        style_search_query,
        editing_style_draft,
        editing_char_style,
        editing_char_draft,
        editing_table_style,
        editing_list_style,
        editing_list_level,
        editing_page_style,
        style_panel_inspect,
    };

    rsx! {
        div {
            style: format!(
                // `position: relative`, and NO `z-index` — so no stacking
                // context. The spelling menu left for `AtPopoverHost` in r63; the
                // comment that named it as a child here outlived it by a commit.
                "display: flex; flex-direction: column; flex: 1; position: relative; \
                 overflow: hidden; background: {bg}; ",
                bg = tokens::COLOR_SURFACE_BASE,
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
                super::editor_keydown::DocShortcuts {
                    on_new: document_actions.on_new,
                    on_open: document_actions.on_open,
                },
                path_signal,
                document_load,
                canvas_hovered,
                page_gap_px,
                spell_service.clone(),
                spell_menu,
                doc_state_spell_ctx,
                zoom_percent,
                zoom_command,
                macro_run_request,
            )}

            // ── Font-substitution detail panel (Spec 03 M3, inverted) ─────────
            // The indicator is the status-bar chip below; this panel opens on
            // demand from that chip. Breakpoint-aware (table vs. card stack);
            // renders nothing while closed or when there are no substitutions.
            super::editor_font_warning::FontSubstitutionPanel {
                substitutions: font_substitutions.clone(),
                open: font_panel_open,
            }
            super::editor_macro_notice::MacroNoticeBar { ctx: super::editor_macro_notice::MacroCtx(doc_state_ribbon.clone()), loro_doc, macro_run_request, save_request }
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

            // ── Style surfaces (inline, above ribbon) ─────────────────────────
            // Their position in this column is load-bearing — see
            // `editor_style_panels` for why they are in flow rather than overlaid.
            {super::editor_style_panels::style_panels(
                doc_state_style_picker,
                doc_state_style_editor,
                style_panel_state,
                editing_table_draft,
                current_style_name.clone(),
                use_breakpoint(),
                font_families(),
                style_sync,
            )}

            // ── Docked panels: spelling menu, language picker, Insert link ────
            // Each self-gates on its trigger signal. Docked above the ribbon
            // in flow; the spelling menu uses position: absolute (confirmed).
            {docked_panels(
                doc_state_docked,
                scroll_offset,
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
            )}

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

            {repair_banner}
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
                        &doc_state_ribbon, loro_doc, cursor_state, open_color_picker,
                        dialogs.span_format, is_char_style_picker_open,
                    ),
                    2 => insert_tab_content(dialogs, insert_ctx.clone()),
                    7 if table_selected => super::editor_ribbon_table::table_tab_content(
                        &doc_state_ribbon, loro_doc, cursor_state, undo_manager, can_undo, can_redo,
                    ),
                    3 => super::editor_ribbon_layout::layout_tab_content(&doc_state_ribbon, loro_doc, cursor_state, undo_manager, can_undo, can_redo, dialogs.page_style, editing_style_draft),
                    4 => super::editor_ribbon_references::references_tab_content(&doc_state_ribbon, loro_doc, cursor_state, undo_manager, can_undo, can_redo),
                    5 => super::editor_ribbon_review::review_tab_content(&doc_state_ribbon, loro_doc, cursor_state, undo_manager, can_undo, can_redo),
                    6 => publish_tab_content(is_publish_panel_open, dialogs),
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
                    paragraph_style_dialog,
                    document_actions,
                    ribbon_recents.read().entries.iter()
                        .map(|e| (e.path.clone(), e.title.clone())).collect(),
                ),
                },
            }

            // ── Status bar ────────────────────────────────────────────────────
            // Extracted to `editor_status_bar` when the zoom badge became the
            // zoom control (Spec 08 T5.4): its props roughly doubled, and this
            // file is baselined over the ceiling and may not grow.
            super::editor_status_bar::EditorStatusBar {
                page_label:         page_label,
                word_count_label:   word_count_label(),
                font_sub_count:     font_sub_count,
                fit_inputs:         zoom_fit_inputs(),
                css_px_per_inch:    display_ppi(),
                zoom_capability_limit_permille: zoom_cap_permille(),
                zoom:               zoom_command,
                view_mode:          view_mode,
                view_mode_user_set: view_mode_user_set,
                font_panel_open:    font_panel_open,
                save_message:       save_message,
                calibrating:        calibrating,
            }

            // Modal overlays — see `editor_modals` for the mounting contract.
            {super::editor_modals::editor_modals(
                doc_state_modals,
                calibrating,
                zoom_command,
                paragraph_style_dialog,
                dialogs,
                font_families(),
                path_signal,
                style_sync,
            )}
        }
    }
}
