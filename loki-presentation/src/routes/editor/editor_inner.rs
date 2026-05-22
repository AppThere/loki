// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Presentation editor inner view.

use appthere_ui::tokens;
use appthere_ui::{AtRibbon, AtRibbonGroup, AtRibbonIconButton, AtStatusBar, RibbonTabDesc};
use dioxus::prelude::*;
use loki_i18n::fl;

use crate::utils::display_title_from_path;

#[derive(Clone, Debug, PartialEq)]
struct Slide {
    title: String,
    subtitle: String,
    bullets: Vec<String>,
    background_color: String,
    text_color: String,
}

impl Default for Slide {
    fn default() -> Self {
        Self {
            title: "New Slide".to_string(),
            subtitle: "Double click to edit subtitle".to_string(),
            bullets: vec!["Point 1".to_string(), "Point 2".to_string()],
            background_color: "#FFFFFF".to_string(),
            text_color: "#1A1A1A".to_string(),
        }
    }
}

/// Presentation editor inner component.
#[component]
pub(super) fn EditorInner(path: String) -> Element {
    let _navigator = use_navigator();
    let title = use_memo(move || display_title_from_path(&path));

    let mut slides = use_signal(|| {
        vec![
            Slide {
                title: "Loki Presentation Suite".to_string(),
                subtitle: "Interactive Slide Deck Editor Shell".to_string(),
                bullets: vec![
                    "Native OS windowing via Dioxus 0.7 & Blitz".to_string(),
                    "Consistent Flat Design token system".to_string(),
                    "Shared loki-i18n translation engine".to_string(),
                ],
                background_color: "#1E1E1E".to_string(),
                text_color: "#FFFFFF".to_string(),
            },
            Slide {
                title: "Premium Features".to_string(),
                subtitle: "Modern, high-performance Blitz GPU rendering".to_string(),
                bullets: vec![
                    "Vector rendering via Vello".to_string(),
                    "Fully isolated package layouts".to_string(),
                    "Responsive 16:9 canvas placeholders".to_string(),
                ],
                background_color: "#FFFFFF".to_string(),
                text_color: "#1A1A1A".to_string(),
            },
            Slide {
                title: "Interactive Editing".to_string(),
                subtitle: "Click text fields on the canvas to edit them live".to_string(),
                bullets: vec![
                    "Select theme profiles in the ribbon".to_string(),
                    "Add or delete slides in the sidebar".to_string(),
                ],
                background_color: "#3D7EFF".to_string(),
                text_color: "#FFFFFF".to_string(),
            },
        ]
    });

    let mut active_slide_idx = use_signal(|| 0usize);
    let mut editing_part: Signal<Option<String>> = use_signal(|| None);

    let active_slide = slides.read()[active_slide_idx()].clone();
    let total_slides = slides.read().len();

    let page_label = fl!(
        "editor-slide-label",
        current = (active_slide_idx() + 1) as i64,
        total = total_slides as i64
    );

    // Theme selector
    let mut apply_theme = move |bg: &str, text: &str| {
        let idx = active_slide_idx();
        let mut sls = slides.write();
        sls[idx].background_color = bg.to_string();
        sls[idx].text_color = text.to_string();
    };



    let mut delete_slide = move |idx: usize| {
        let mut sls = slides.write();
        if sls.len() <= 1 {
            return; // Don't delete the last slide
        }
        sls.remove(idx);
        let active = active_slide_idx();
        if active >= sls.len() {
            active_slide_idx.set(sls.len() - 1);
        } else if active == idx && active > 0 {
            active_slide_idx.set(active - 1);
        }
        editing_part.set(None);
    };

    let home_tab = rsx! {
        AtRibbonGroup {
            label:      None,
            aria_label: "Slides Management".to_string(),

            AtRibbonIconButton {
                icon_label:  "+".to_string(),
                aria_label:  "Add Slide".to_string(),
                is_active:   false,
                is_disabled: false,
                on_click: move |_| {
                    let mut sls = slides.write();
                    let new_idx = sls.len();
                    sls.push(Slide::default());
                    active_slide_idx.set(new_idx);
                    editing_part.set(None);
                },
            }
            AtRibbonIconButton {
                icon_label:  "\u{2212}".to_string(),
                aria_label:  "Delete Slide".to_string(),
                is_active:   false,
                is_disabled: total_slides <= 1,
                on_click: move |_| {
                    delete_slide(active_slide_idx());
                },
            }
        }

        AtRibbonGroup {
            label:      None,
            aria_label: "Slide Themes".to_string(),

            AtRibbonIconButton {
                icon_label: "Dark".to_string(),
                aria_label: "Dark Theme".to_string(),
                is_active:  active_slide.background_color == "#1E1E1E",
                is_disabled: false,
                on_click: move |_| {
                    apply_theme("#1E1E1E", "#FFFFFF");
                },
            }

            AtRibbonIconButton {
                icon_label: "Light".to_string(),
                aria_label: "Light Theme".to_string(),
                is_active:  active_slide.background_color == "#FFFFFF",
                is_disabled: false,
                on_click: move |_| {
                    apply_theme("#FFFFFF", "#1A1A1A");
                },
            }

            AtRibbonIconButton {
                icon_label: "Blue".to_string(),
                aria_label: "Blue Accent Theme".to_string(),
                is_active:  active_slide.background_color == "#3D7EFF",
                is_disabled: false,
                on_click: move |_| {
                    apply_theme("#3D7EFF", "#FFFFFF");
                },
            }

            AtRibbonIconButton {
                icon_label: "Beige".to_string(),
                aria_label: "Warm Beige Theme".to_string(),
                is_active:  active_slide.background_color == "#FAF6EE",
                is_disabled: false,
                on_click: move |_| {
                    apply_theme("#FAF6EE", "#4A3525");
                },
            }
        }
    };

    rsx! {
        div {
            style: format!(
                "display: flex; flex-direction: column; flex: 1; \
                 overflow: hidden; background: {bg}; font-family: system-ui, sans-serif;",
                bg = tokens::COLOR_SURFACE_BASE,
            ),

            // ── Title Bar / File Info indicator ──────────────────────────────
            div {
                style: "display: flex; flex-direction: row; justify-content: space-between; align-items: center; \
                        padding: 6px 16px; background: #1E1E1E; border-bottom: 1px solid #3A3A3A;",
                span {
                    style: "font-size: 13px; font-weight: bold; color: #E8E8E8;",
                    "{title}"
                }
                span {
                    style: "font-size: 11px; color: #888888;",
                    "Local File • PPTX / ODP"
                }
            }

            // ── Sidebar + Canvas Area ────────────────────────────────────────
            div {
                style: "flex: 1; display: flex; flex-direction: row; overflow: hidden;",

                // Left Sidebar: Thumbnails list
                div {
                    style: "width: 180px; background: #252525; border-right: 1px solid #3A3A3A; \
                            display: flex; flex-direction: column; overflow-y: auto; padding: 12px; gap: 12px;",

                    for (idx, slide) in slides.read().iter().enumerate() {
                        div {
                            key: "{idx}",
                            style: format!(
                                "position: relative; width: 100%; height: 90px; \
                                 background: {bg}; border: 2px solid {border}; \
                                 border-radius: 6px; cursor: pointer; padding: 8px; \
                                 box-sizing: border-box; display: flex; flex-direction: column; \
                                 justify-content: space-between; transition: border-color 0.2s;",
                                bg = slide.background_color,
                                border = if idx == active_slide_idx() { "#3D7EFF" } else { "#444444" },
                            ),
                            onclick: move |_| {
                                active_slide_idx.set(idx);
                                editing_part.set(None);
                            },

                            // Delete indicator
                            button {
                                style: "position: absolute; top: 4px; right: 4px; border: none; \
                                        background: rgba(0, 0, 0, 0.4); color: white; border-radius: 50%; \
                                        width: 16px; height: 16px; font-size: 10px; cursor: pointer; \
                                        display: flex; align-items: center; justify-content: center;",
                                onclick: move |e| {
                                    e.stop_propagation();
                                    delete_slide(idx);
                                },
                                "×"
                            }

                            span {
                                style: format!(
                                    "font-size: 10px; font-weight: bold; overflow: hidden; \
                                     text-overflow: ellipsis; white-space: nowrap; color: {fg};",
                                    fg = slide.text_color
                                ),
                                "{slide.title}"
                            }

                            span {
                                style: "font-size: 10px; color: #888888; align-self: flex-start;",
                                "{idx + 1}"
                            }
                        }
                    }

                    // Add Slide Card
                    button {
                        style: "width: 100%; height: 40px; background: #333333; border: 1px dashed #555555; \
                                border-radius: 6px; color: #E8E8E8; font-size: 12px; cursor: pointer; \
                                display: flex; align-items: center; justify-content: center; gap: 6px;",
                        onclick: move |_| {
                            let mut sls = slides.write();
                            let new_idx = sls.len();
                            sls.push(Slide::default());
                            active_slide_idx.set(new_idx);
                            editing_part.set(None);
                        },
                        span { style: "font-size: 16px;", "+" }
                        "Add Slide"
                    }
                }

                // Center workspace: Slide view
                div {
                    style: "flex: 1; display: flex; align-items: center; justify-content: center; padding: 24px; overflow: auto;",

                    // 16:9 Interactive Slide Canvas
                    div {
                        style: format!(
                            "width: 720px; height: 405px; background: {bg}; color: {text_color}; \
                             border-radius: 8px; box-shadow: 0 4px 12px rgba(0,0,0,0.15); \
                             padding: 40px; box-sizing: border-box; display: flex; flex-direction: column; \
                             justify-content: flex-start; position: relative;",
                            bg = active_slide.background_color,
                            text_color = active_slide.text_color,
                        ),

                        // Title field
                        if editing_part() == Some("title".to_string()) {
                            input {
                                style: "font-size: 32px; font-weight: bold; background: transparent; \
                                        border: 1px dashed #888888; color: inherit; width: 100%; \
                                        outline: none; font-family: inherit;",
                                value: "{active_slide.title}",
                                autofocus: true,
                                oninput: move |e| {
                                    let mut sls = slides.write();
                                    sls[active_slide_idx()].title = e.value();
                                },
                                onblur: move |_| { editing_part.set(None); },
                                onkeydown: move |e| {
                                    if e.key() == Key::Enter {
                                        editing_part.set(None);
                                    }
                                }
                            }
                        } else {
                            h1 {
                                style: "font-size: 32px; font-weight: bold; margin: 0; cursor: text; \
                                        border: 1px solid transparent; border-radius: 4px; padding: 2px;",
                                onclick: move |_| { editing_part.set(Some("title".to_string())); },
                                "{active_slide.title}"
                            }
                        }

                        // Subtitle field
                        if editing_part() == Some("subtitle".to_string()) {
                            input {
                                style: "font-size: 16px; font-style: italic; background: transparent; \
                                        border: 1px dashed #888888; color: inherit; width: 100%; \
                                        outline: none; font-family: inherit; margin-top: 10px;",
                                value: "{active_slide.subtitle}",
                                autofocus: true,
                                oninput: move |e| {
                                    let mut sls = slides.write();
                                    sls[active_slide_idx()].subtitle = e.value();
                                },
                                onblur: move |_| { editing_part.set(None); },
                                onkeydown: move |e| {
                                    if e.key() == Key::Enter {
                                        editing_part.set(None);
                                    }
                                }
                            }
                        } else {
                            p {
                                style: "font-size: 16px; font-style: italic; margin: 10px 0 0 0; cursor: text; \
                                        opacity: 0.85; border: 1px solid transparent; border-radius: 4px; padding: 2px;",
                                onclick: move |_| { editing_part.set(Some("subtitle".to_string())); },
                                "{active_slide.subtitle}"
                            }
                        }

                        // Divider line
                        div {
                            style: format!(
                                "height: 2px; width: 80px; background: {color}; margin: 24px 0; opacity: 0.5;",
                                color = active_slide.text_color
                            )
                        }

                        // Bullet points
                        ul {
                            style: "margin: 0; padding-left: 20px; flex: 1;",

                            for (b_idx, bullet) in active_slide.bullets.iter().enumerate() {
                                {
                                    let key = format!("bullet-{}", b_idx);
                                    let is_editing = editing_part() == Some(key.clone());

                                    rsx! {
                                        li {
                                            style: "margin: 8px 0; font-size: 16px; cursor: text; list-style-type: square;",
                                            if is_editing {
                                                input {
                                                    style: "background: transparent; border: 1px dashed #888888; \
                                                            color: inherit; width: 90%; outline: none; \
                                                            font-size: 16px; font-family: inherit;",
                                                    value: "{bullet}",
                                                    autofocus: true,
                                                    oninput: move |e| {
                                                        let mut sls = slides.write();
                                                        sls[active_slide_idx()].bullets[b_idx] = e.value();
                                                    },
                                                    onblur: move |_| { editing_part.set(None); },
                                                    onkeydown: move |e| {
                                                        if e.key() == Key::Enter {
                                                            editing_part.set(None);
                                                        }
                                                    }
                                                }
                                            } else {
                                                span {
                                                    style: "border: 1px solid transparent; border-radius: 4px; padding: 2px;",
                                                    onclick: move |_| { editing_part.set(Some(key.clone())); },
                                                    "{bullet}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Add bullet item
                            button {
                                style: "background: none; border: none; color: inherit; opacity: 0.6; \
                                        cursor: pointer; font-size: 12px; margin-top: 10px; display: flex; \
                                        align-items: center; gap: 4px; padding: 4px;",
                                onclick: move |_| {
                                    let mut sls = slides.write();
                                    sls[active_slide_idx()].bullets.push("New bullet point".to_string());
                                },
                                span { style: "font-size: 14px;", "+" }
                                "Add Bullet Point"
                            }
                        }
                    }
                }
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
                on_tab_select: move |_idx| {},
                tab_content: home_tab
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
