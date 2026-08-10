// SPDX-License-Identifier: Apache-2.0

//! The span dialog's **Highlight** and **Language** tabs.

use appthere_ui::{AtDialogNotice, AtField, AtNoticeTone, DialogPosture, at_control_style, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::body::{SpanDraftSignal, StyleContext, chip_style, edit, grid, line, span_all};

/// The **Highlight** tab.
pub(super) fn highlight(
    draft: SpanDraftSignal,
    posture: DialogPosture,
    styles: &StyleContext,
) -> Element {
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let m = current.marks.clone();
    // The model's named highlight colours, restricted to the muted end: the
    // design's five tints are chosen to stay legible at 1-bit on E-Ink, and the
    // saturated half of the palette does not.
    const COLOURS: [(&str, &str); 5] = [
        ("Yellow", "#F2E3B8"),
        ("Green", "#D8E7DA"),
        ("Red", "#E9D7CE"),
        ("Cyan", "#D6DFE9"),
        ("Magenta", "#E4DCE9"),
    ];

    rsx! {
        div {
            style: grid(posture),

            AtField {
                label: fl!("span-dialog-highlight-colour"),
                extra_style: span_all(posture),
                control: rsx! {
                    div {
                        style: format!(
                            "display: flex; flex-direction: row; flex-wrap: wrap; gap: {gap}px;",
                            gap = tokens::SPACE_2,
                        ),
                        button {
                            style: chip_style(m.highlight.is_none(), posture),
                            onclick: move |evt| {
                                evt.stop_propagation();
                                edit(draft, |d| d.marks.highlight = None);
                            },
                            { fl!("span-dialog-none") }
                        }
                        for (name, swatch) in COLOURS.iter() {
                            button {
                                key: "{name}",
                                aria_label: "{name}",
                                style: format!(
                                    "width: 56px; height: 44px; border-radius: {r}px; \
                                     cursor: pointer; background: {swatch}; border: {bw} solid {bc};",
                                    r = tokens::RADIUS_MD,
                                    bw = if m.highlight.as_deref() == Some(name) { "2px" } else { "1px" },
                                    bc = if m.highlight.as_deref() == Some(name) {
                                        tokens::COLOR_TAB_ACTIVE_INDICATOR
                                    } else {
                                        tokens::COLOR_BORDER_CHROME
                                    },
                                ),
                                onclick: move |evt| {
                                    evt.stop_propagation();
                                    edit(draft, move |d| {
                                        d.marks.highlight = Some((*name).to_string());
                                    });
                                },
                            }
                        }
                    }
                },
                footnote: line(
                    m.highlight.is_some(),
                    m.highlight.clone(),
                    styles,
                    posture,
                    draft,
                    |d| d.marks.highlight = None,
                ),
            }

            AtDialogNotice {
                tone: AtNoticeTone::Info,
                extra_style: span_all(posture),
                message: rsx! { { fl!("span-dialog-highlight-export") } },
            }
        }
    }
}

/// The **Language** tab.
pub(super) fn language(
    draft: SpanDraftSignal,
    posture: DialogPosture,
    styles: &StyleContext,
) -> Element {
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let m = current.marks.clone();
    let value = m.language.clone().unwrap_or_default();

    rsx! {
        div {
            style: grid(posture),

            AtField {
                label: fl!("span-dialog-language"),
                extra_style: span_all(posture),
                control: rsx! {
                    div {
                        style: at_control_style(posture.min_touch_px, "width: 100%;"),
                        input {
                            r#type: "text",
                            value: "{value}",
                            placeholder: fl!("span-dialog-language-placeholder"),
                            style: format!(
                                "flex: 1; min-width: 0; background: transparent; border: none; \
                                 font-size: {fs}px; color: {fg};",
                                fs = tokens::FONT_SIZE_MD,
                                fg = tokens::COLOR_TEXT_ON_CHROME,
                            ),
                            oninput: move |evt| {
                                let text = evt.value();
                                edit(draft, move |d| {
                                    let t = text.trim();
                                    d.marks.language =
                                        (!t.is_empty()).then(|| t.to_string());
                                });
                            },
                        }
                    }
                },
                footnote: line(
                    m.language.is_some(),
                    m.language.clone(),
                    styles,
                    posture,
                    draft,
                    |d| d.marks.language = None,
                ),
            }

            // The highest-value accessibility control in the dialog: a marked
            // run exports as `<span xml:lang="fr-FR">`, so a screen reader
            // switches voice mid-sentence.
            AtDialogNotice {
                tone: AtNoticeTone::Positive,
                extra_style: span_all(posture),
                message: rsx! { { fl!("span-dialog-language-note") } },
            }
        }
    }
}
