// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Right-hand edit form rows for the style editor panel.

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::super::editor_state::StyleDraft;

/// Renders the "Name" text-field row.
pub(super) fn name_row(mut editing_style_draft: Signal<Option<StyleDraft>>, name: String) -> Element {
    rsx! {
        div {
            style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
            span {
                style: format!(
                    "font-family: {ff}; font-size: {fs}px; color: {fg}; min-width: 64px;",
                    ff = tokens::FONT_FAMILY_UI,
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                "Name"
            }
            input {
                r#type: "text",
                value: "{name}",
                oninput: move |evt| {
                    let v = editing_style_draft.read().clone();
                    if let Some(mut d) = v { d.name = evt.value(); editing_style_draft.set(Some(d)); }
                },
                style: format!(
                    "flex: 1; height: 24px; padding: 0 {p}px; \
                     background: {bg}; border: 1px solid {border}; \
                     border-radius: {r}px; font-family: {ff}; \
                     font-size: {fs}px; color: {fg}; box-sizing: border-box;",
                    p = tokens::SPACE_2,
                    bg = tokens::COLOR_SURFACE_2,
                    border = tokens::COLOR_BORDER_DEFAULT,
                    r = tokens::RADIUS_SM,
                    ff = tokens::FONT_FAMILY_UI,
                    fs = tokens::FONT_SIZE_BODY,
                    fg = tokens::COLOR_TEXT_ON_CHROME,
                ),
            }
        }
    }
}

/// Renders the "Based on" and "Next style" paired text-field row.
pub(super) fn parent_next_row(
    mut editing_style_draft: Signal<Option<StyleDraft>>,
    parent: String,
    next: String,
) -> Element {
    rsx! {
        div {
            style: "display: flex; flex-direction: row; align-items: center; gap: 16px;",
            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 8px; flex: 1;",
                span {
                    style: format!("font-family: {ff}; font-size: {fs}px; color: {fg}; min-width: 56px;", ff = tokens::FONT_FAMILY_UI, fs = tokens::FONT_SIZE_LABEL, fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY),
                    { fl!("editor-style-based-on-label") }
                }
                input {
                    r#type: "text",
                    value: "{parent}",
                    oninput: {
                        let mut esd = editing_style_draft;
                        move |evt| {
                            let v = esd.read().clone();
                            if let Some(mut d) = v { d.parent = evt.value(); esd.set(Some(d)); }
                        }
                    },
                    style: format!("flex: 1; height: 24px; padding: 0 {p}px; background: {bg}; border: 1px solid {border}; border-radius: {r}px; font-family: {ff}; font-size: {fs}px; color: {fg}; box-sizing: border-box;", p = tokens::SPACE_2, bg = tokens::COLOR_SURFACE_2, border = tokens::COLOR_BORDER_DEFAULT, r = tokens::RADIUS_SM, ff = tokens::FONT_FAMILY_UI, fs = tokens::FONT_SIZE_BODY, fg = tokens::COLOR_TEXT_ON_CHROME),
                }
            }
            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 8px; flex: 1;",
                span {
                    style: format!("font-family: {ff}; font-size: {fs}px; color: {fg}; min-width: 56px;", ff = tokens::FONT_FAMILY_UI, fs = tokens::FONT_SIZE_LABEL, fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY),
                    { fl!("editor-style-next-style-label") }
                }
                input {
                    r#type: "text",
                    value: "{next}",
                    oninput: move |evt| {
                        let v = editing_style_draft.read().clone();
                        if let Some(mut d) = v { d.next = evt.value(); editing_style_draft.set(Some(d)); }
                    },
                    style: format!("flex: 1; height: 24px; padding: 0 {p}px; background: {bg}; border: 1px solid {border}; border-radius: {r}px; font-family: {ff}; font-size: {fs}px; color: {fg}; box-sizing: border-box;", p = tokens::SPACE_2, bg = tokens::COLOR_SURFACE_2, border = tokens::COLOR_BORDER_DEFAULT, r = tokens::RADIUS_SM, ff = tokens::FONT_FAMILY_UI, fs = tokens::FONT_SIZE_BODY, fg = tokens::COLOR_TEXT_ON_CHROME),
                }
            }
        }
    }
}

/// Renders the alignment button strip, font-size input, and B/I/U toggle row.
pub(super) fn format_row(
    mut editing_style_draft: Signal<Option<StyleDraft>>,
    draft_alignment: String,
    font_size_str: String,
    biu: [(&'static str, bool); 3],
) -> Element {
    rsx! {
        div {
            style: "display: flex; flex-direction: row; align-items: center; gap: 16px; flex-wrap: wrap;",

            // Alignment buttons
            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 4px;",
                span {
                    style: format!("font-family: {ff}; font-size: {fs}px; color: {fg}; margin-right: 4px;", ff = tokens::FONT_FAMILY_UI, fs = tokens::FONT_SIZE_LABEL, fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY),
                    { fl!("editor-style-align-label") }
                }
                {["Left", "Center", "Right", "Justify"].iter().map(|&aval| {
                    let is_a = draft_alignment.as_str() == aval;
                    rsx! {
                        button {
                            key: "{aval}",
                            style: format!(
                                "padding: 2px 6px; border-radius: 3px; border: 1px solid {border}; \
                                 cursor: pointer; font-family: {ff}; font-size: {fs}px; \
                                 background: {bg}; color: {fg};",
                                border = if is_a { tokens::COLOR_TAB_ACTIVE_INDICATOR } else { tokens::COLOR_BORDER_CHROME },
                                ff = tokens::FONT_FAMILY_UI,
                                fs = tokens::FONT_SIZE_LABEL,
                                bg = if is_a { tokens::COLOR_SURFACE_3 } else { tokens::COLOR_SURFACE_2 },
                                fg = tokens::COLOR_TEXT_ON_CHROME,
                            ),
                            onclick: move |_| {
                                let v = editing_style_draft.read().clone();
                                if let Some(mut d) = v { d.alignment = aval.to_string(); editing_style_draft.set(Some(d)); }
                            },
                            "{aval}"
                        }
                    }
                })}
            }

            // Font size input
            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 4px;",
                span {
                    style: format!("font-family: {ff}; font-size: {fs}px; color: {fg};", ff = tokens::FONT_FAMILY_UI, fs = tokens::FONT_SIZE_LABEL, fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY),
                    "Size (pt)"
                }
                input {
                    r#type: "text",
                    value: "{font_size_str}",
                    oninput: move |evt| {
                        let v = editing_style_draft.read().clone();
                        if let Some(mut d) = v { d.font_size_str = evt.value(); editing_style_draft.set(Some(d)); }
                    },
                    style: format!("width: 40px; height: 24px; padding: 0 {p}px; background: {bg}; border: 1px solid {border}; border-radius: {r}px; font-family: {ff}; font-size: {fs}px; color: {fg}; box-sizing: border-box;", p = tokens::SPACE_2, bg = tokens::COLOR_SURFACE_2, border = tokens::COLOR_BORDER_DEFAULT, r = tokens::RADIUS_SM, ff = tokens::FONT_FAMILY_UI, fs = tokens::FONT_SIZE_BODY, fg = tokens::COLOR_TEXT_ON_CHROME),
                }
            }

            // B/I/U toggle buttons
            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 4px;",
                {biu.into_iter().map(|(lbl, active)| {
                    rsx! {
                        button {
                            key: "{lbl}",
                            style: format!(
                                "padding: 2px 8px; border-radius: 3px; border: 1px solid {border}; \
                                 cursor: pointer; font-family: {ff}; font-size: {fs}px; \
                                 font-weight: {fw}; font-style: {fi}; text-decoration: {td}; \
                                 background: {bg}; color: {fg};",
                                border = if active { tokens::COLOR_TAB_ACTIVE_INDICATOR } else { tokens::COLOR_BORDER_CHROME },
                                ff = tokens::FONT_FAMILY_UI,
                                fs = tokens::FONT_SIZE_LABEL,
                                fw = if lbl == "B" { "700" } else { "400" },
                                fi = if lbl == "I" { "italic" } else { "normal" },
                                td = if lbl == "U" { "underline" } else { "none" },
                                bg = if active { tokens::COLOR_SURFACE_3 } else { tokens::COLOR_SURFACE_2 },
                                fg = tokens::COLOR_TEXT_ON_CHROME,
                            ),
                            onclick: move |_| {
                                let v = editing_style_draft.read().clone();
                                if let Some(mut d) = v {
                                    match lbl {
                                        "B" => d.bold = !d.bold,
                                        "I" => d.italic = !d.italic,
                                        "U" => d.underline = !d.underline,
                                        _ => {}
                                    }
                                    editing_style_draft.set(Some(d));
                                }
                            },
                            "{lbl}"
                        }
                    }
                })}
            }
        }
    }
}
