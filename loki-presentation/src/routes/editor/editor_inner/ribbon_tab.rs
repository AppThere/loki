// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Home ribbon tab content for the presentation editor.

use appthere_ui::{AtRibbonGroup, AtRibbonIconButton};
use dioxus::prelude::*;

use super::slide::Slide;

#[derive(Props, Clone, PartialEq)]
pub(super) struct HomeRibbonTabProps {
    pub(super) slides: Signal<Vec<Slide>>,
    pub(super) active_slide_idx: Signal<usize>,
    pub(super) editing_part: Signal<Option<String>>,
    pub(super) total_slides: usize,
    pub(super) active_bg: String,
}

/// Home ribbon tab: slide management and theme selector buttons.
///
/// Minimum touch target: each icon button is at least 44×44 logical pixels
/// (enforced by `AtRibbonIconButton`).
#[component]
pub(super) fn HomeRibbonTab(props: HomeRibbonTabProps) -> Element {
    let HomeRibbonTabProps {
        mut slides,
        mut active_slide_idx,
        mut editing_part,
        total_slides,
        active_bg,
    } = props;

    let mut delete_slide = move |idx: usize| {
        let mut sls = slides.write();
        if sls.len() <= 1 {
            return;
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

    let mut apply_theme = move |bg: &str, text: &str| {
        let idx = active_slide_idx();
        let mut sls = slides.write();
        sls[idx].background_color = bg.to_string();
        sls[idx].text_color = text.to_string();
    };

    rsx! {
        AtRibbonGroup {
            label:      None,
            aria_label: "Slides Management".to_string(),

            AtRibbonIconButton {
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
                span { "+" }
            }
            AtRibbonIconButton {
                aria_label:  "Delete Slide".to_string(),
                is_active:   false,
                is_disabled: total_slides <= 1,
                on_click: move |_| {
                    delete_slide(active_slide_idx());
                },
                span { "\u{2212}" }
            }
        }

        AtRibbonGroup {
            label:      None,
            aria_label: "Slide Themes".to_string(),

            AtRibbonIconButton {
                aria_label: "Dark Theme".to_string(),
                is_active:  active_bg == "#1E1E1E",
                is_disabled: false,
                on_click: move |_| { apply_theme("#1E1E1E", "#FFFFFF"); },
                span { "Dark" }
            }
            AtRibbonIconButton {
                aria_label: "Light Theme".to_string(),
                is_active:  active_bg == "#FFFFFF",
                is_disabled: false,
                on_click: move |_| { apply_theme("#FFFFFF", "#1A1A1A"); },
                span { "Light" }
            }
            AtRibbonIconButton {
                aria_label: "Blue Accent Theme".to_string(),
                is_active:  active_bg == "#3D7EFF",
                is_disabled: false,
                on_click: move |_| { apply_theme("#3D7EFF", "#FFFFFF"); },
                span { "Blue" }
            }
            AtRibbonIconButton {
                aria_label: "Warm Beige Theme".to_string(),
                is_active:  active_bg == "#FAF6EE",
                is_disabled: false,
                on_click: move |_| { apply_theme("#FAF6EE", "#4A3525"); },
                span { "Beige" }
            }
        }
    }
}
