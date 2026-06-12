// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Document editor — inner component.
//!
//! [`EditorInner`] holds all per-document hook state and renders the editor
//! layout: ribbon, scrollable page canvas, and status bar.  Document
//! switching is handled reactively via `path_signal` — see `editor_path_sync`
//! for the full design.

mod font_warning;
mod hooks;
mod save_banner;

use std::sync::Arc;

use appthere_ui::tokens;
use appthere_ui::{AtRibbon, AtStatusBar, RibbonTabDesc};
use dioxus::prelude::*;
use loki_doc_model::document::Document;
use loki_i18n::fl;

use font_warning::{build_font_data, font_warning_banner};
use hooks::{make_formatting_effect, make_loro_init_effect, make_page_count_effect};
use save_banner::save_message_banner;

use super::editor_canvas::render_canvas_area;
use super::editor_load::load_document;
use super::editor_path_sync::sync_path_and_reset;
use super::editor_ribbon::home_tab_content;
use super::editor_state::{EditorState, use_editor_state};
use super::editor_style::style_picker_panel;
use super::editor_style_editor::style_editor_panel;
use crate::error::LoadError;

/// Document editor inner component — all editing logic lives here.
///
/// Document switching is handled reactively via `path_signal`.
/// EditorMode was removed — the editor is always in edit mode.
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
        mut current_page,
        mut total_pages,
        bold_active,
        italic_active,
        underline_active,
        strikethrough_active,
        superscript_active,
        subscript_active,
        mut undo_manager,
        mut can_undo,
        mut can_redo,
        mut is_style_picker_open,
        mut editing_style_draft,
        mut save_message,
    } = use_editor_state();

    // ── Synchronous Path Sync & State Reset ──────────────────────────────────
    sync_path_and_reset(
        &path,
        &mut path_signal,
        &doc_state,
        &mut cursor_state,
        &mut loro_doc,
        &mut undo_manager,
        &mut total_pages,
        &mut current_page,
        &mut can_undo,
        &mut can_redo,
        &mut dismiss_font_warning,
        &mut is_style_picker_open,
        &mut editing_style_draft,
        &mut save_message,
    );

    // Current paragraph style — computed from signals, always current.
    let current_style_name = {
        let cs = cursor_state.read();
        let ldoc = loro_doc.read();
        if let (Some(l), Some(focus)) = (ldoc.as_ref(), cs.focus.as_ref()) {
            loki_doc_model::get_block_style_name(l, focus.paragraph_index)
        } else {
            String::new()
        }
    };

    // Clone Arc handles — each closure below captures one owned clone.
    let (doc_state_mousedown, doc_state_mousemove) =
        (Arc::clone(&doc_state), Arc::clone(&doc_state));
    let (doc_state_touch, doc_state_touchend) = (Arc::clone(&doc_state), Arc::clone(&doc_state));
    let (doc_state_keydown, doc_state_pages) = (Arc::clone(&doc_state), Arc::clone(&doc_state));
    let (doc_state_ribbon, doc_state_style_picker) =
        (Arc::clone(&doc_state), Arc::clone(&doc_state));
    let (doc_state_style_editor, doc_state_seed) = (Arc::clone(&doc_state), Arc::clone(&doc_state));
    let doc_state_render = Arc::clone(&doc_state);

    // ── Document load — reactive on path_signal ───────────────────────────────
    let document_load: Resource<(String, Result<Document, LoadError>)> = use_resource(move || {
        let p = path_signal();
        async move {
            let res = load_document(p.clone());
            (p, res)
        }
    });

    let _navigator = use_navigator();

    // ── Reactive effects — see hooks.rs for closure implementations ──────────
    use_effect(make_loro_init_effect(
        doc_state_seed,
        loro_doc,
        undo_manager,
        cursor_state,
        path_signal,
        document_load,
    ));
    use_effect(make_page_count_effect(
        doc_state_pages,
        document_load,
        total_pages,
    ));
    use_effect(make_formatting_effect(
        cursor_state,
        loro_doc,
        bold_active,
        italic_active,
        underline_active,
        strikethrough_active,
        superscript_active,
        subscript_active,
    ));

    // TODO(scroll): Blitz convert_scroll_data is unimplemented! — onscroll
    // cannot be used safely; current_page stays at 1 until Blitz fixes this.
    let _ = scroll_offset;

    let canvas_hovered = use_signal(|| false);
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

    // ── Font substitution data ────────────────────────────────────────────────
    let font_data = build_font_data(&doc_state);

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
                is_dragging,
                drag_origin,
                touch_state,
                window_width,
                scroll_offset,
                cursor_state,
                loro_doc,
                undo_manager,
                can_undo,
                can_redo,
                path_signal,
                document_load,
                canvas_hovered,
                page_gap_px,
            )}

            // ── Font Warning Banner ──────────────────────────────────────────
            if !font_data.substitutions.is_empty() && !dismiss_font_warning() {
                {font_warning_banner(font_data.warning_details, font_data.download_links, dismiss_font_warning)}
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

            // ── Save message banner ───────────────────────────────────────────
            if let Some(msg) = save_message.read().clone() {
                {save_message_banner(msg, save_message)}
            }

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
                collapsed: ribbon_collapsed(),
                on_toggle_collapse: move |_| {
                    ribbon_collapsed.set(!ribbon_collapsed());
                },
                toggle_aria_label: if ribbon_collapsed() {
                    fl!("ribbon-expand-aria")
                } else {
                    fl!("ribbon-collapse-aria")
                },
                tab_content: home_tab_content(
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
