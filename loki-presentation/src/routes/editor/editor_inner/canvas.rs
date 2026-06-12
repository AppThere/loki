// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Interactive 16:9 slide canvas with inline text editing.

use dioxus::prelude::*;

use super::slide::Slide;

#[derive(Props, Clone, PartialEq)]
pub(super) struct SlideCanvasProps {
    pub(super) slides: Signal<Vec<Slide>>,
    pub(super) active_slide_idx: Signal<usize>,
    pub(super) editing_part: Signal<Option<String>>,
}

/// 16:9 interactive slide canvas. Click any text field to edit inline.
///
/// Minimum touch target: text areas are at least 44×44 logical pixels.
#[component]
pub(super) fn SlideCanvas(props: SlideCanvasProps) -> Element {
    let SlideCanvasProps {
        mut slides,
        active_slide_idx,
        mut editing_part,
    } = props;

    let active_slide = slides.read()[active_slide_idx()].clone();

    rsx! {
        div {
            style: "flex: 1; display: flex; align-items: center; justify-content: center; padding: 24px; overflow: auto;",

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

                    // Add bullet item — min 44×44 touch target
                    button {
                        style: "background: none; border: none; color: inherit; opacity: 0.6; \
                                cursor: pointer; font-size: 12px; margin-top: 10px; display: flex; \
                                align-items: center; gap: 4px; padding: 4px; min-height: 44px;",
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
}
