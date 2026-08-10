// SPDX-License-Identifier: Apache-2.0

//! The span dialog's **Font** tab: family, size, weight and slant.

use std::rc::Rc;

use appthere_ui::{AtDialogNotice, AtField, AtNoticeTone, DialogPosture, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;

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
    let families: Vec<String> = font_families.iter().take(60).cloned().collect();
    let selected_family = m
        .font_family
        .as_ref()
        .and_then(|f| families.iter().position(|c| c == f))
        .unwrap_or(usize::MAX);

    rsx! {
        div {
            style: grid(posture),

            AtField {
                label: fl!("span-dialog-font-family"),
                extra_style: span_all(posture),
                control: rsx! {
                    div {
                        style: format!(
                            "display: flex; flex-direction: row; flex-wrap: wrap; gap: {gap}px;",
                            gap = tokens::SPACE_2,
                        ),
                        // Bundled faces first, badged — the same rule the
                        // paragraph picker follows, so a face chosen in either
                        // place carries the same guarantee.
                        for f in loki_fonts::bundled_families().iter() {
                            button {
                                key: "{f.name}",
                                style: chip_style(
                                    m.font_family.as_deref() == Some(f.name),
                                    posture,
                                ),
                                onclick: move |evt| {
                                    evt.stop_propagation();
                                    edit(draft, move |d| {
                                        d.marks.font_family = Some(f.name.to_string());
                                    });
                                },
                                {f.name}
                            }
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

            if selected_family == usize::MAX && m.font_family.is_some() {
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
