// SPDX-License-Identifier: Apache-2.0

//! The span dialog's **Font** tab: family, size, weight and slant.

use std::rc::Rc;

use appthere_ui::{AtDialogNotice, AtField, AtNoticeTone, DialogPosture, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::super::font_family_field::{FontFamilyPicker, FontFamilyPickerProps};
use super::body::{
    SpanDraftSignal, StyleContext, chip_style, edit, grid, line, number_field, span_all, tri_toggle,
};
use super::format_number;

/// The **Font** tab.
pub(super) fn font(
    draft: SpanDraftSignal,
    posture: DialogPosture,
    styles: &StyleContext,
    font_families: Rc<Vec<String>>,
) -> Element {
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let m = current.marks.clone();
    // Whether the face travels with the file. Keyed off the bundled list, not
    // off `families` — that is a truncated slice of whatever this device has
    // installed, so it raised the substitution caution for the six faces that
    // ship inside Loki (the one class with no substitution risk) and stayed
    // quiet for a device face that happened to sort into the first sixty.
    let substitutes = m
        .font_family
        .as_deref()
        .is_some_and(|f| !loki_fonts::is_bundled_family(f));

    rsx! {
        div {
            style: grid(posture),

            // The reusable span-level styles (LibreOffice's character styles):
            // apply one from the catalog, or None to fall back to the
            // paragraph level. A style level, not direct formatting — Clear
            // direct formatting leaves it in place (design note 09).
            AtField {
                label: fl!("span-dialog-char-style"),
                extra_style: span_all(posture),
                control: rsx! {
                    if styles.char_styles.is_empty() {
                        div {
                            style: format!(
                                "font-size: {fs}px; color: {fg};",
                                fs = tokens::FONT_SIZE_BODY,
                                fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                            ),
                            { fl!("span-dialog-char-style-empty") }
                        }
                    } else {
                        div {
                            style: format!(
                                "display: flex; flex-direction: row; flex-wrap: wrap; gap: {gap}px;",
                                gap = tokens::SPACE_2,
                            ),
                            button {
                                style: chip_style(current.char_style.is_none(), posture),
                                onclick: move |evt| {
                                    evt.stop_propagation();
                                    edit(draft, |d| d.char_style = None);
                                },
                                { fl!("span-dialog-char-style-none") }
                            }
                            for (id, display) in styles.char_styles.iter().cloned() {
                                button {
                                    key: "{id}",
                                    style: chip_style(
                                        current.char_style.as_deref() == Some(id.as_str()),
                                        posture,
                                    ),
                                    onclick: {
                                        let id = id.clone();
                                        move |evt: Event<MouseData>| {
                                            evt.stop_propagation();
                                            let id = id.clone();
                                            edit(draft, move |d| {
                                                d.char_style = Some(id.clone());
                                            });
                                        }
                                    },
                                    {display.clone()}
                                }
                            }
                        }
                    }
                },
            }

            AtField {
                label: fl!("span-dialog-font-family"),
                extra_style: span_all(posture),
                // The same searchable bundled-first picker the paragraph
                // dialog uses, so a face chosen in either place carries the
                // same guarantee — and every installed family is reachable
                // (the old chip grid stopped at sixty, silently).
                control: rsx! {
                    FontFamilyPicker {
                        ..FontFamilyPickerProps {
                            selected: m.font_family.clone(),
                            placeholder: fl!("style-dialog-font-family-inherit"),
                            font_families: Rc::clone(&font_families),
                            posture,
                            on_pick: EventHandler::new(move |name: String| {
                                edit(draft, move |d| {
                                    d.marks.font_family = Some(name.clone());
                                });
                            }),
                        }
                    }
                },
                footnote: line(
                    m.font_family.is_some(),
                    m.font_family.clone(),
                    styles,
                    posture,
                    draft,
                    |d| d.marks.font_family = None,
                ),
            }

            { number_field(
                fl!("span-dialog-size"),
                current.size_buffer.clone(),
                fl!("span-dialog-unit-pt"),
                posture,
                draft,
                |d, v| {
                    // A zero or negative type size is not a size.
                    if let Ok(n) = v.trim().parse::<f64>() {
                        if n > 0.0 {
                            d.marks.font_size_pt = Some(n);
                        }
                    } else if v.trim().is_empty() {
                        d.marks.font_size_pt = None;
                    }
                    d.size_buffer = v;
                },
                line(
                    m.font_size_pt.is_some(),
                    m.font_size_pt.map(format_number),
                    styles,
                    posture,
                    draft,
                    |d| {
                        d.marks.font_size_pt = None;
                        d.size_buffer = String::new();
                    },
                ),
            ) }

            AtField {
                label: fl!("span-dialog-style"),
                control: rsx! {
                    div {
                        style: "display: flex; flex-direction: column;",
                        { tri_toggle(fl!("span-dialog-bold"), m.bold, posture, draft, |d, v| d.marks.bold = v) }
                        { tri_toggle(fl!("span-dialog-italic"), m.italic, posture, draft, |d, v| d.marks.italic = v) }
                    }
                },
                footnote: line(
                    m.bold.is_some() || m.italic.is_some(),
                    None,
                    styles,
                    posture,
                    draft,
                    |d| {
                        d.marks.bold = None;
                        d.marks.italic = None;
                    },
                ),
            }

            { number_field(
                fl!("span-dialog-letter-spacing"),
                current.spacing_buffer.clone(),
                fl!("span-dialog-unit-pt"),
                posture,
                draft,
                |d, v| {
                    if let Ok(n) = v.trim().parse::<f64>() {
                        d.marks.letter_spacing_pt = Some(n);
                    } else if v.trim().is_empty() {
                        d.marks.letter_spacing_pt = None;
                    }
                    d.spacing_buffer = v;
                },
                line(
                    m.letter_spacing_pt.is_some(),
                    m.letter_spacing_pt.map(format_number),
                    styles,
                    posture,
                    draft,
                    |d| {
                        d.marks.letter_spacing_pt = None;
                        d.spacing_buffer = String::new();
                    },
                ),
            ) }

            if substitutes {
                AtDialogNotice {
                    tone: AtNoticeTone::Caution,
                    extra_style: span_all(posture),
                    message: rsx! {
                        {
                            fl!(
                                "span-dialog-device-face",
                                name = m.font_family.clone().unwrap_or_default()
                            )
                        }
                    },
                }
            }
        }
    }
}
