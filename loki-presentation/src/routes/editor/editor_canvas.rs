// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The editable slide canvas component.

use dioxus::prelude::*;
use loki_graphics::ShapeId;
use loki_i18n::fl;

use super::slide_view::SlideView;

/// A text edit: set paragraph `para` of `shape_id` to `text`.
#[derive(Clone)]
pub(super) struct EditMsg {
    /// The shape being edited.
    pub shape_id: ShapeId,
    /// Paragraph index within the shape.
    pub para: usize,
    /// The new text.
    pub text: String,
}

/// The slide thumbnail rail. Emits the index to select on click.
#[component]
pub(super) fn SlideThumbnails(
    views: Vec<SlideView>,
    active: usize,
    on_select: EventHandler<usize>,
) -> Element {
    rsx! {
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
                        border = if i == active { "#3D7EFF" } else { "#444444" },
                    ),
                    onclick: move |_| on_select.call(i),
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
    }
}

/// The text shown on a slide thumbnail: the title, else the first non-empty
/// bullet, else a generic slide label.
fn thumbnail_label(view: &SlideView, index: usize) -> String {
    if let Some(t) = &view.title
        && !t.text.is_empty()
    {
        return t.text.clone();
    }
    if let Some(first) = view.bullets.iter().find(|b| !b.text.is_empty()) {
        return first.text.clone();
    }
    fl!(
        "editor-slide-label",
        current = (index + 1) as i64,
        total = (index + 1) as i64
    )
}

/// Renders one slide's flattened content as editable fields on a 16:9 canvas.
#[component]
pub(super) fn SlideCanvas(
    view: SlideView,
    on_edit: EventHandler<EditMsg>,
    on_add_bullet: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            style: format!(
                "width: 720px; height: 405px; background: {bg}; color: {fg}; \
                 border-radius: 8px; box-shadow: 0 4px 12px rgba(0,0,0,0.15); \
                 padding: 40px; box-sizing: border-box; display: flex; \
                 flex-direction: column; justify-content: flex-start; gap: 8px;",
                bg = view.bg_css,
                fg = view.fg_css,
            ),

            if let Some(t) = &view.title {
                input {
                    style: "font-size: 32px; font-weight: bold; background: transparent; \
                            border: 1px dashed rgba(128,128,128,0.4); color: inherit; \
                            width: 100%; outline: none; font-family: inherit; padding: 2px;",
                    value: "{t.text}",
                    placeholder: fl!("editor-placeholder-title"),
                    oninput: {
                        let id = t.shape_id.clone();
                        move |e: Event<FormData>| {
                            on_edit.call(EditMsg { shape_id: id.clone(), para: 0, text: e.value() });
                        }
                    },
                }
            }

            if let Some(s) = &view.subtitle {
                input {
                    style: "font-size: 16px; font-style: italic; background: transparent; \
                            border: 1px dashed rgba(128,128,128,0.4); color: inherit; \
                            width: 100%; outline: none; font-family: inherit; padding: 2px; opacity: 0.85;",
                    value: "{s.text}",
                    placeholder: fl!("editor-placeholder-subtitle"),
                    oninput: {
                        let id = s.shape_id.clone();
                        move |e: Event<FormData>| {
                            on_edit.call(EditMsg { shape_id: id.clone(), para: 0, text: e.value() });
                        }
                    },
                }
            }

            ul {
                style: "margin: 12px 0 0 0; padding-left: 20px; flex: 1; overflow-y: auto;",
                for (i, line) in view.bullets.iter().enumerate() {
                    li {
                        key: "{i}",
                        style: "margin: 6px 0; font-size: 16px; list-style-type: square;",
                        input {
                            style: "background: transparent; border: 1px dashed rgba(128,128,128,0.4); \
                                    color: inherit; width: 92%; outline: none; font-size: 16px; \
                                    font-family: inherit; padding: 2px;",
                            value: "{line.text}",
                            oninput: {
                                let id = line.shape_id.clone();
                                let para = line.para;
                                move |e: Event<FormData>| {
                                    on_edit.call(EditMsg { shape_id: id.clone(), para, text: e.value() });
                                }
                            },
                        }
                    }
                }
                button {
                    style: "background: none; border: none; color: inherit; opacity: 0.6; \
                            cursor: pointer; font-size: 12px; margin-top: 8px; padding: 4px;",
                    onclick: move |_| on_add_bullet.call(()),
                    {fl!("editor-action-add-bullet")}
                }
            }
        }
    }
}
