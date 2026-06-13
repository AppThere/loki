// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Presentation editor inner view.
//!
//! Loads the presentation from the route `path` via the PPTX importer and
//! renders the real slides read-only. Faithful per-shape positioning needs the
//! GPU slide canvas; until then [`super::slide_view`] flattens each slide to a
//! title / subtitle / bullet flow (Blitz does not support absolute
//! positioning). Editing and saving are tracked follow-ups.

use appthere_ui::AtStatusBar;
use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_error_view::EditorErrorView;
use super::editor_load::load_presentation;
use super::slide_view::{SlideView, slide_views};
use crate::utils::display_title_from_path;

/// Presentation editor inner component.
#[component]
pub(super) fn EditorInner(path: String) -> Element {
    let title = use_memo({
        let path = path.clone();
        move || display_title_from_path(&path)
    });
    let mut active_idx = use_signal(|| 0usize);

    // Load (and re-load on path change) the presentation off the file token.
    let load = use_resource({
        let path = path.clone();
        move || {
            let path = path.clone();
            async move { load_presentation(path) }
        }
    });

    // Extract render-ready, owned data from the resource so the borrow is
    // released before building the view.
    let value = load.value();
    let (views, error): (Vec<SlideView>, Option<String>) = match &*value.read_unchecked() {
        None => (Vec::new(), None),
        Some(Ok(pres)) => (slide_views(pres), None),
        Some(Err(e)) => (Vec::new(), Some(e.to_string())),
    };

    if let Some(message) = error {
        return rsx! {
            EditorErrorView { message: fl!("editor-load-failed", reason = message) }
        };
    }

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

            // ── Title bar ────────────────────────────────────────────────────
            div {
                style: "display: flex; flex-direction: row; justify-content: space-between; \
                        align-items: center; padding: 6px 16px; background: #1E1E1E; \
                        border-bottom: 1px solid #3A3A3A;",
                span {
                    style: "font-size: 13px; font-weight: bold; color: #E8E8E8;",
                    "{title}"
                }
                span {
                    style: "font-size: 11px; color: #888888;",
                    "Local File • PPTX"
                }
            }

            // ── Read-only banner ─────────────────────────────────────────────
            div {
                style: "flex-shrink: 0; padding: 8px 16px; background: #1A3A4A; \
                        border-bottom: 1px solid #2A5A6A; color: #B0E0F0; font-size: 12px;",
                {fl!("editor-presentation-readonly")}
            }

            // ── Sidebar + canvas ─────────────────────────────────────────────
            div {
                style: "flex: 1; display: flex; flex-direction: row; overflow: hidden;",

                // Slide thumbnails
                div {
                    style: "width: 180px; background: #252525; border-right: 1px solid #3A3A3A; \
                            display: flex; flex-direction: column; overflow-y: auto; \
                            padding: 12px; gap: 12px;",
                    for (i, v) in views.iter().enumerate() {
                        div {
                            key: "{i}",
                            style: format!(
                                "position: relative; width: 100%; height: 90px; background: {bg}; \
                                 border: 2px solid {border}; border-radius: 6px; cursor: pointer; \
                                 padding: 8px; box-sizing: border-box; display: flex; \
                                 flex-direction: column; justify-content: space-between;",
                                bg = v.bg_css,
                                border = if i == idx { "#3D7EFF" } else { "#444444" },
                            ),
                            onclick: move |_| active_idx.set(i),
                            span {
                                style: format!(
                                    "font-size: 10px; font-weight: bold; overflow: hidden; \
                                     text-overflow: ellipsis; white-space: nowrap; color: {fg};",
                                    fg = v.fg_css,
                                ),
                                {thumbnail_label(v, i)}
                            }
                            span {
                                style: "font-size: 10px; color: #888888; align-self: flex-start;",
                                "{i + 1}"
                            }
                        }
                    }
                }

                // Center canvas
                div {
                    style: "flex: 1; display: flex; align-items: center; justify-content: center; \
                            padding: 24px; overflow: auto;",
                    if let Some(v) = active {
                        SlideCanvas { view: v }
                    } else {
                        span {
                            style: "color: #888888; font-size: 14px;",
                            {fl!("editor-presentation-empty")}
                        }
                    }
                }
            }

            // ── Status bar ───────────────────────────────────────────────────
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

/// Renders a single slide's flattened content onto a 16:9 canvas.
#[component]
fn SlideCanvas(view: SlideView) -> Element {
    rsx! {
        div {
            style: format!(
                "width: 720px; height: 405px; background: {bg}; color: {fg}; \
                 border-radius: 8px; box-shadow: 0 4px 12px rgba(0,0,0,0.15); \
                 padding: 40px; box-sizing: border-box; display: flex; \
                 flex-direction: column; justify-content: flex-start;",
                bg = view.bg_css,
                fg = view.fg_css,
            ),
            if !view.title.is_empty() {
                h1 {
                    style: "font-size: 32px; font-weight: bold; margin: 0;",
                    "{view.title}"
                }
            }
            if !view.subtitle.is_empty() {
                p {
                    style: "font-size: 16px; font-style: italic; margin: 10px 0 0 0; opacity: 0.85;",
                    "{view.subtitle}"
                }
            }
            if !view.bullets.is_empty() {
                ul {
                    style: "margin: 24px 0 0 0; padding-left: 20px; flex: 1;",
                    for (i, bullet) in view.bullets.iter().enumerate() {
                        li {
                            key: "{i}",
                            style: "margin: 8px 0; font-size: 16px; list-style-type: square;",
                            "{bullet}"
                        }
                    }
                }
            }
        }
    }
}

/// The text shown on a slide thumbnail: the title, else the first bullet, else a
/// generic "Slide N".
fn thumbnail_label(view: &SlideView, index: usize) -> String {
    if !view.title.is_empty() {
        view.title.clone()
    } else if let Some(first) = view.bullets.first() {
        first.clone()
    } else {
        fl!(
            "editor-slide-label",
            current = (index + 1) as i64,
            total = (index + 1) as i64
        )
    }
}
