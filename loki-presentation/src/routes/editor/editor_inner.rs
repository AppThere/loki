// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Presentation editor inner view.
//!
//! Loads a presentation from the route `path` (PPTX importer), holds it in an
//! editable signal, and writes it back via the PPTX exporter on Save / Save As.
//! Text editing flattens each slide to title/subtitle/bullets (Blitz has no
//! absolute positioning); faithful per-shape placement is the GPU-canvas
//! follow-up. Each edit writes back to the exact shape the value came from (see
//! [`super::slide_view`] / [`super::edit`]).

use appthere_ui::AtStatusBar;
use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_file_access::{FileAccessToken, FilePicker, SaveOptions};
use loki_i18n::fl;
use loki_presentation_model::Presentation;

use super::edit;
use super::editor_canvas::{EditMsg, SlideCanvas, SlideThumbnails};
use super::editor_error_view::EditorErrorView;
use super::editor_load::load_presentation;
use super::editor_save::export_to_token;
use super::slide_view::slide_views;
use crate::new_document::is_untitled;
use crate::recent_documents::RecentDocuments;
use crate::routes::Route;
use crate::tabs::OpenTab;
use crate::utils::display_title_from_path;

const PPTX_MIME: &str = "application/vnd.openxmlformats-officedocument.presentationml.presentation";

/// Presentation editor inner component.
#[component]
pub(super) fn EditorInner(path: String) -> Element {
    let navigator = use_navigator();
    let mut tabs = use_context::<Signal<Vec<OpenTab>>>();
    let recent_docs = use_context::<Signal<RecentDocuments>>();

    let mut path_signal = use_signal(|| path.clone());
    let mut doc = use_signal(|| Option::<Presentation>::None);
    let mut load_error = use_signal(|| Option::<String>::None);
    let mut active_idx = use_signal(|| 0usize);
    let mut dirty = use_signal(|| false);
    let mut save_message = use_signal(|| Option::<String>::None);

    // Reset per-document state when the route path changes (tab switch / Save As
    // navigation reuses this component instance).
    if *path_signal.peek() != path {
        path_signal.set(path.clone());
        doc.set(None);
        load_error.set(None);
        active_idx.set(0);
        dirty.set(false);
        save_message.set(None);
    }

    // Load reactively on path; populate the editable doc once resolved.
    let load = use_resource(move || async move { load_presentation(path_signal()) });
    use_effect(move || {
        if doc.peek().is_some() || load_error.peek().is_some() {
            return;
        }
        match &*load.value().read_unchecked() {
            Some(Ok(p)) => doc.set(Some(p.clone())),
            Some(Err(e)) => load_error.set(Some(e.to_string())),
            None => {}
        }
    });

    // Mirror the dirty flag onto the tab indicator.
    use_effect(move || {
        let d = dirty();
        let p = path_signal.peek().clone();
        let mut t = tabs.write();
        if let Some(tab) = t.iter_mut().find(|tb| tb.path == p)
            && tab.is_dirty != d
        {
            tab.is_dirty = d;
        }
    });

    let title = use_memo(move || display_title_from_path(&path_signal()));

    // ── Save As ───────────────────────────────────────────────────────────────
    let save_as = use_callback(move |_: ()| {
        let Some(pres) = doc.peek().clone() else {
            return;
        };
        let cur_path = path_signal.peek().clone();
        let suggested = format!("{}.pptx", display_title_from_path(&cur_path));
        let mut tabs = tabs;
        let mut recent = recent_docs;
        let nav = navigator;
        spawn(async move {
            let picker = FilePicker::new();
            let opts = SaveOptions {
                mime_type: Some(PPTX_MIME.to_string()),
                suggested_name: Some(suggested),
            };
            match picker.pick_file_to_save(opts).await {
                Ok(Some(token)) => match export_to_token(&token, &pres) {
                    Ok(()) => {
                        let new_path = token.serialize();
                        let new_title = display_title_from_path(&new_path);
                        {
                            let mut t = tabs.write();
                            if let Some(tab) = t.iter_mut().find(|tb| tb.path == cur_path) {
                                tab.path = new_path.clone();
                                tab.title = new_title.clone();
                                tab.is_dirty = false;
                            }
                        }
                        recent.write().record(new_path.clone(), new_title);
                        recent.read().save();
                        dirty.set(false);
                        save_message.set(Some(fl!("editor-save-success")));
                        nav.push(Route::Editor { path: new_path });
                    }
                    Err(e) => save_message.set(Some(fl!("editor-save-error", reason = e))),
                },
                Ok(None) => {}
                Err(e) => save_message.set(Some(fl!("editor-save-error", reason = e.to_string()))),
            }
        });
    });

    // ── Save ──────────────────────────────────────────────────────────────────
    let save = use_callback(move |_: ()| {
        let cur = path_signal.peek().clone();
        if is_untitled(&cur) {
            save_as.call(());
            return;
        }
        let Some(pres) = doc.peek().clone() else {
            return;
        };
        match FileAccessToken::deserialize(&cur) {
            Ok(token) => match export_to_token(&token, &pres) {
                Ok(()) => {
                    dirty.set(false);
                    save_message.set(Some(fl!("editor-save-success")));
                }
                Err(e) => save_message.set(Some(fl!("editor-save-error", reason = e))),
            },
            Err(e) => save_message.set(Some(fl!("editor-save-error", reason = e.to_string()))),
        }
    });

    // ── Render states ──────────────────────────────────────────────────────────
    if let Some(message) = load_error() {
        return rsx! {
            EditorErrorView { message: fl!("editor-load-failed", reason = message) }
        };
    }
    let Some(pres) = doc() else {
        return rsx! {
            div {
                style: "flex: 1; display: flex; align-items: center; justify-content: center; \
                        color: #888888; font-size: 14px;",
                {fl!("editor-presentation-loading")}
            }
        };
    };

    let views = slide_views(&pres);
    let total = views.len();
    let idx = active_idx().min(total.saturating_sub(1));
    let active = views.get(idx).cloned();

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; flex: 1; overflow: hidden; \
                 background: {bg}; font-family: system-ui, sans-serif;",
                bg = tokens::COLOR_SURFACE_BASE,
            ),

            // ── Toolbar ────────────────────────────────────────────────────────
            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 8px; \
                        padding: 6px 16px; background: #1E1E1E; border-bottom: 1px solid #3A3A3A;",
                span {
                    style: "font-size: 13px; font-weight: bold; color: #E8E8E8; margin-right: 8px;",
                    "{title}"
                }
                button {
                    style: toolbar_btn_style(),
                    onclick: move |_| save.call(()),
                    {fl!("editor-action-save")}
                }
                button {
                    style: toolbar_btn_style(),
                    onclick: move |_| {
                        if let Some(p) = doc.write().as_mut() { edit::add_slide(p); }
                        dirty.set(true);
                    },
                    {fl!("editor-action-add-slide")}
                }
                button {
                    style: toolbar_btn_style(),
                    onclick: move |_| {
                        if let Some(p) = doc.write().as_mut() { edit::delete_slide(p, idx); }
                        dirty.set(true);
                    },
                    {fl!("editor-action-delete-slide")}
                }
            }

            // ── Save status banner ─────────────────────────────────────────────
            if let Some(msg) = save_message() {
                div {
                    style: "display: flex; flex-direction: row; justify-content: space-between; \
                            align-items: center; padding: 6px 16px; background: #2A3A4A; \
                            border-bottom: 1px solid #3A4A5A; color: #DCEAF6; font-size: 12px;",
                    span { "{msg}" }
                    button {
                        style: "background: none; border: none; color: #DCEAF6; cursor: pointer; \
                                font-size: 14px; padding: 0 4px;",
                        aria_label: fl!("editor-dismiss-aria"),
                        // Icon-only (×) control: expose a hover tooltip via the
                        // blitz-shell overlay (reads `title`).
                        title:      fl!("editor-dismiss-aria"),
                        onclick: move |_| save_message.set(None),
                        "\u{00D7}"
                    }
                }
            }

            // ── Sidebar + canvas ───────────────────────────────────────────────
            div {
                style: "flex: 1; display: flex; flex-direction: row; overflow: hidden;",

                SlideThumbnails {
                    views: views.clone(),
                    active: idx,
                    on_select: move |i: usize| active_idx.set(i),
                }

                div {
                    style: "flex: 1; display: flex; align-items: center; justify-content: center; \
                            padding: 24px; overflow: auto;",
                    if let Some(v) = active {
                        SlideCanvas {
                            view: v,
                            on_edit: move |msg: EditMsg| {
                                if let Some(p) = doc.write().as_mut() {
                                    edit::set_shape_text(p, idx, &msg.shape_id, msg.para, &msg.text);
                                }
                                dirty.set(true);
                            },
                            on_add_bullet: move |()| {
                                if let Some(p) = doc.write().as_mut() { edit::add_bullet(p, idx); }
                                dirty.set(true);
                            },
                        }
                    } else {
                        span {
                            style: "color: #888888; font-size: 14px;",
                            {fl!("editor-presentation-empty")}
                        }
                    }
                }
            }

            // ── Status bar ─────────────────────────────────────────────────────
            AtStatusBar {
                page_label: fl!(
                    "editor-slide-label",
                    current = (idx + 1).min(total.max(1)) as i64,
                    total = total as i64
                ),
                word_count_label: String::new(),
                language_label: fl!("editor-language"),
                zoom_percent: 100,
                collaborator_count: 0,
                collaborator_label: String::new(),
                zoom_aria_label: fl!("editor-zoom-aria"),
                on_zoom_click: |_| {},
            }
        }
    }
}

fn toolbar_btn_style() -> &'static str {
    "padding: 4px 10px; background: #333333; border: 1px solid #555555; border-radius: 4px; \
     color: #E8E8E8; font-size: 12px; cursor: pointer;"
}
