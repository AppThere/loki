// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Presentation editor inner view.

mod canvas;
mod ribbon_tab;
mod sidebar;
mod slide;

use appthere_ui::tokens;
use appthere_ui::{AtRibbon, AtStatusBar, RibbonTabDesc};
use dioxus::prelude::*;
use loki_i18n::fl;

use canvas::SlideCanvas;
use ribbon_tab::HomeRibbonTab;
use sidebar::SlideSidebar;
use slide::Slide;

use crate::utils::display_title_from_path;

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

    let active_slide_idx = use_signal(|| 0usize);
    let editing_part: Signal<Option<String>> = use_signal(|| None);

    let active_bg = slides.read()[active_slide_idx()].background_color.clone();
    let total_slides = slides.read().len();

    let page_label = fl!(
        "editor-slide-label",
        current = (active_slide_idx() + 1) as i64,
        total = total_slides as i64
    );

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

            // ── Sidebar + Canvas Area ─────────────────────────────────────────
            div {
                style: "flex: 1; display: flex; flex-direction: row; overflow: hidden;",

                SlideSidebar {
                    slides,
                    active_slide_idx,
                    editing_part,
                }

                SlideCanvas {
                    slides,
                    active_slide_idx,
                    editing_part,
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
                collapsed: false,
                on_toggle_collapse: move |_| {},
                toggle_aria_label: fl!("ribbon-collapse-aria"),
                on_tab_select: move |_idx| {},
                tab_content: rsx! {
                    HomeRibbonTab {
                        slides,
                        active_slide_idx,
                        editing_part,
                        total_slides,
                        active_bg,
                    }
                }
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
