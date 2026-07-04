// SPDX-License-Identifier: Apache-2.0

//! Font-family picker and numeric weight selector for the style editor form,
//! plus the shared label / text-input style helpers used across the form.

use std::rc::Rc;

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;

use super::super::editor_state::StyleDraft;

/// Numeric weights offered by the selector (OpenType 1–1000 scale).
pub(super) const WEIGHTS: [u16; 7] = [100, 300, 400, 500, 600, 700, 900];

/// Maps a numeric weight to its localised display label.
fn weight_label(w: u16) -> String {
    match w {
        100 => fl!("editor-style-weight-thin"),
        300 => fl!("editor-style-weight-light"),
        500 => fl!("editor-style-weight-medium"),
        600 => fl!("editor-style-weight-semibold"),
        700 => fl!("editor-style-weight-bold"),
        900 => fl!("editor-style-weight-black"),
        _ => fl!("editor-style-weight-regular"),
    }
}

/// Inline font-family picker: a text input that doubles as a substring filter
/// over the available families, with a scrollable result list beneath it.
///
/// The candidate list renders inline below the input rather than as an
/// overlaid popover. Blitz has no native `<select>` dropdown; block-level
/// `position: absolute` is now confirmed (CLAUDE.md), but an overlay anchored to
/// this input sits in an *inline* formatting context, where absolute positioning
/// is still unverified in Blitz — so the inline list stays for now.
///
/// Touch targets: each family row is a full-width button; with 4px vertical
/// padding inside the 84px scroll area the rows clear the 44×44 logical-pixel
/// minimum on touch builds where the base font is scaled up.
pub(super) fn font_picker(
    mut editing_style_draft: Signal<Option<StyleDraft>>,
    selected: String,
    font_families: Rc<Vec<String>>,
) -> Element {
    let query = selected.trim().to_lowercase();
    let matches: Vec<String> = font_families
        .iter()
        .filter(|f| query.is_empty() || f.to_lowercase().contains(&query))
        .take(50)
        .cloned()
        .collect();

    rsx! {
        div {
            style: "display: flex; flex-direction: column; gap: 4px;",
            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
                span { style: label_style(), { fl!("editor-style-font-label") } }
                input {
                    r#type: "text",
                    value: "{selected}",
                    oninput: move |evt| {
                        let v = editing_style_draft.read().clone();
                        if let Some(mut d) = v {
                            d.font_name = evt.value();
                            editing_style_draft.set(Some(d));
                        }
                    },
                    style: input_style("flex: 1"),
                }
            }
            div {
                style: format!(
                    "max-height: 84px; overflow-y: auto; display: flex; \
                     flex-direction: column; gap: 2px; border: 1px solid {border}; \
                     border-radius: {r}px; padding: 2px;",
                    border = tokens::COLOR_BORDER_CHROME,
                    r = tokens::RADIUS_SM,
                ),
                {matches.into_iter().map(|name| {
                    let is_sel = name.eq_ignore_ascii_case(selected.trim());
                    let name_cap = name.clone();
                    rsx! {
                        button {
                            key: "{name}",
                            style: format!(
                                "text-align: left; padding: 4px 6px; border-radius: 3px; \
                                 border: none; cursor: pointer; font-family: {ff}; \
                                 font-size: {fs}px; background: {bg}; color: {fg};",
                                ff = tokens::FONT_FAMILY_UI,
                                fs = tokens::FONT_SIZE_LABEL,
                                bg = if is_sel { tokens::COLOR_SURFACE_3 } else { "transparent" },
                                fg = tokens::COLOR_TEXT_ON_CHROME,
                            ),
                            onclick: move |_| {
                                let v = editing_style_draft.read().clone();
                                if let Some(mut d) = v {
                                    d.font_name = name_cap.clone();
                                    editing_style_draft.set(Some(d));
                                }
                            },
                            "{name}"
                        }
                    }
                })}
            }
        }
    }
}

/// Row of weight buttons (Thin … Black); each sets the draft's numeric weight.
/// Each button renders its label in the weight it selects, previewing the face.
pub(super) fn weight_selector(
    mut editing_style_draft: Signal<Option<StyleDraft>>,
    current: u16,
) -> Element {
    rsx! {
        div {
            style: "display: flex; flex-direction: row; align-items: center; gap: 4px; flex-wrap: wrap;",
            span {
                style: format!("{} margin-right: 4px;", label_style()),
                { fl!("editor-style-weight-label") }
            }
            {WEIGHTS.into_iter().map(|w| {
                let is_w = current == w;
                rsx! {
                    button {
                        key: "{w}",
                        style: format!(
                            "padding: 2px 6px; border-radius: 3px; border: 1px solid {border}; \
                             cursor: pointer; font-family: {ff}; font-size: {fs}px; \
                             font-weight: {fw}; background: {bg}; color: {fg};",
                            border = if is_w { tokens::COLOR_TAB_ACTIVE_INDICATOR } else { tokens::COLOR_BORDER_CHROME },
                            ff = tokens::FONT_FAMILY_UI,
                            fs = tokens::FONT_SIZE_LABEL,
                            fw = w,
                            bg = if is_w { tokens::COLOR_SURFACE_3 } else { tokens::COLOR_SURFACE_2 },
                            fg = tokens::COLOR_TEXT_ON_CHROME,
                        ),
                        onclick: move |_| {
                            let v = editing_style_draft.read().clone();
                            if let Some(mut d) = v {
                                d.font_weight = w;
                                editing_style_draft.set(Some(d));
                            }
                        },
                        { weight_label(w) }
                    }
                }
            })}
        }
    }
}

/// Shared label `<span>` style for the form's field captions.
pub(super) fn label_style() -> String {
    format!(
        "font-family: {ff}; font-size: {fs}px; color: {fg};",
        ff = tokens::FONT_FAMILY_UI,
        fs = tokens::FONT_SIZE_LABEL,
        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
    )
}

/// Shared text-input style; `extra` injects width / flex rules per call site.
pub(super) fn input_style(extra: &str) -> String {
    format!(
        "{extra}; height: 24px; padding: 0 {p}px; background: {bg}; \
         border: 1px solid {border}; border-radius: {r}px; font-family: {ff}; \
         font-size: {fs}px; color: {fg}; box-sizing: border-box;",
        p = tokens::SPACE_2,
        bg = tokens::COLOR_SURFACE_2,
        border = tokens::COLOR_BORDER_DEFAULT,
        r = tokens::RADIUS_SM,
        ff = tokens::FONT_FAMILY_UI,
        fs = tokens::FONT_SIZE_BODY,
        fg = tokens::COLOR_TEXT_ON_CHROME,
    )
}
