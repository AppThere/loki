// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Slide panel sidebar (thumbnail list + add-slide button).

use dioxus::prelude::*;

use super::slide::Slide;

#[derive(Props, Clone, PartialEq)]
pub(super) struct SlideSidebarProps {
    pub(super) slides: Signal<Vec<Slide>>,
    pub(super) active_slide_idx: Signal<usize>,
    pub(super) editing_part: Signal<Option<String>>,
}

/// Left sidebar showing slide thumbnails.
///
/// Minimum touch target: each thumbnail is at least 44×44 logical pixels.
#[component]
pub(super) fn SlideSidebar(props: SlideSidebarProps) -> Element {
    let SlideSidebarProps {
        mut slides,
        mut active_slide_idx,
        mut editing_part,
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

    rsx! {
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

                    // Delete button — 44×44 touch area via absolute positioning
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

            // Add Slide button — min 44×44 touch target
            button {
                style: "width: 100%; height: 44px; background: #333333; border: 1px dashed #555555; \
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
    }
}
