// SPDX-License-Identifier: Apache-2.0

//! The span dialog's **Effects** and **Position** tabs.
//!
//! Both carry a notice naming the controls the document model has no field for
//! — outline and shadow here, raise/lower and rotation next door. Rendering
//! them disabled with the reason stated beats rendering them live and dropping
//! what the user typed.

use appthere_ui::{AtDialogNotice, AtField, AtNoticeTone, AtSegmented, DialogPosture};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::body::{SpanDraftSignal, StyleContext, edit, grid, line, span_all, tri_toggle};

/// The **Effects** tab.
pub(super) fn effects(
    draft: SpanDraftSignal,
    posture: DialogPosture,
    styles: &StyleContext,
) -> Element {
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let m = current.marks.clone();
    let underline_on = m.underline.is_some();
    let strike_on = m.strikethrough.is_some();

    rsx! {
        div {
            style: grid(posture),

            AtField {
                label: fl!("span-dialog-underline"),
                control: rsx! {
                    AtSegmented {
                        options: vec![fl!("span-dialog-none"), fl!("span-dialog-single")],
                        selected: usize::from(underline_on),
                        min_touch_px: posture.min_touch_px,
                        on_select: move |idx: usize| {
                            edit(draft, move |d| {
                                d.marks.underline =
                                    (idx == 1).then(|| "Single".to_string());
                            });
                        },
                    }
                },
                footnote: line(
                    underline_on,
                    m.underline.clone(),
                    styles,
                    posture,
                    draft,
                    |d| d.marks.underline = None,
                ),
            }

            AtField {
                label: fl!("span-dialog-strikethrough"),
                control: rsx! {
                    AtSegmented {
                        options: vec![fl!("span-dialog-none"), fl!("span-dialog-single")],
                        selected: usize::from(strike_on),
                        min_touch_px: posture.min_touch_px,
                        on_select: move |idx: usize| {
                            edit(draft, move |d| {
                                d.marks.strikethrough =
                                    (idx == 1).then(|| "Single".to_string());
                            });
                        },
                    }
                },
                footnote: line(
                    strike_on,
                    m.strikethrough.clone(),
                    styles,
                    posture,
                    draft,
                    |d| d.marks.strikethrough = None,
                ),
            }

            AtField {
                label: fl!("span-dialog-case"),
                extra_style: span_all(posture),
                control: rsx! {
                    div {
                        style: "display: flex; flex-direction: column;",
                        { tri_toggle(fl!("span-dialog-small-caps"), m.small_caps, posture, draft, |d, v| d.marks.small_caps = v) }
                        { tri_toggle(fl!("span-dialog-all-caps"), m.all_caps, posture, draft, |d, v| d.marks.all_caps = v) }
                    }
                },
                footnote: line(
                    m.small_caps.is_some() || m.all_caps.is_some(),
                    None,
                    styles,
                    posture,
                    draft,
                    |d| {
                        d.marks.small_caps = None;
                        d.marks.all_caps = None;
                    },
                ),
            }

            // Outline, emboss and shadow are deliberately not offered: Blitz
            // paints none of them, and an effect that silently disappears on
            // export is worse than one that was never available.
            AtDialogNotice {
                tone: AtNoticeTone::Info,
                extra_style: span_all(posture),
                message: rsx! { { fl!("span-dialog-effects-unsupported") } },
            }
        }
    }
}

/// The **Position** tab.
pub(super) fn position(
    draft: SpanDraftSignal,
    posture: DialogPosture,
    styles: &StyleContext,
) -> Element {
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let m = current.marks.clone();
    const POSITIONS: [&str; 3] = ["Baseline", "Superscript", "Subscript"];
    let selected = m
        .vertical_align
        .as_deref()
        .and_then(|v| POSITIONS.iter().position(|p| *p == v))
        .unwrap_or(0);

    rsx! {
        div {
            style: grid(posture),

            AtField {
                label: fl!("span-dialog-vertical-position"),
                extra_style: span_all(posture),
                control: rsx! {
                    AtSegmented {
                        options: vec![
                            fl!("span-dialog-normal"),
                            fl!("span-dialog-superscript"),
                            fl!("span-dialog-subscript"),
                        ],
                        selected,
                        min_touch_px: posture.min_touch_px,
                        on_select: move |idx: usize| {
                            edit(draft, move |d| {
                                // Baseline is the default, so choosing it clears
                                // the mark rather than pinning "Baseline" as a
                                // direct override of a style that already says so.
                                d.marks.vertical_align = (idx != 0)
                                    .then(|| POSITIONS[idx.min(2)].to_string());
                            });
                        },
                    }
                },
                footnote: line(
                    m.vertical_align.is_some(),
                    m.vertical_align.clone(),
                    styles,
                    posture,
                    draft,
                    |d| d.marks.vertical_align = None,
                ),
            }

            // Raise/lower, relative size, scale-width and rotation have no mark
            // in the Loro schema and no field in `CharProps` that survives the
            // bridge, so they are named rather than drawn.
            // TODO(span-position-detail): add `baseline_shift`, `scale` and a
            // rotation mark to the schema, then wire them here.
            AtDialogNotice {
                tone: AtNoticeTone::Info,
                extra_style: span_all(posture),
                message: rsx! { { fl!("span-dialog-position-unsupported") } },
            }
        }
    }
}
