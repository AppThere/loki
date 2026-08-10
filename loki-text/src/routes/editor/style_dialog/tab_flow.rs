// SPDX-License-Identifier: Apache-2.0

//! The **Text flow** tab: how the paragraph behaves at a page break.

use appthere_ui::tokens;
use appthere_ui::{AtCheckRow, AtDialogNotice, AtNoticeTone, DialogPosture};
use dioxus::prelude::*;
use loki_doc_model::style::{ParagraphStyle, StyleCatalog, StyleId};
use loki_i18n::fl;

use super::draft::{ParaDialogDraft, parse_lines};
use super::fields::{DraftSignal, OpenSignal, body_grid_style, full_width, numeric_field};

/// Renders the Text flow tab body.
pub(super) fn body(
    catalog: &StyleCatalog,
    id: &StyleId,
    draft: DraftSignal,
    open_style: OpenSignal,
    posture: DialogPosture,
) -> Element {
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let pp = current.style.para_props.clone();

    // Each flag shows its **resolved** state, so an inherited "keep together"
    // reads as on rather than as unset.
    let resolved = |local: Option<bool>, get: fn(&ParagraphStyle) -> Option<bool>| -> bool {
        local
            .or_else(|| catalog.resolve_para_chain(id, get).and_then(|r| r.value))
            .unwrap_or(false)
    };
    let keep_together = resolved(pp.keep_together, |s| s.para_props.keep_together);
    let keep_with_next = resolved(pp.keep_with_next, |s| s.para_props.keep_with_next);
    let break_before = resolved(pp.page_break_before, |s| s.para_props.page_break_before);
    let break_after = resolved(pp.page_break_after, |s| s.para_props.page_break_after);
    let lines = || Some(fl!("style-dialog-unit-lines"));

    rsx! {
        div {
            style: body_grid_style(posture),

            { section_heading(fl!("style-dialog-flow-breaks-heading"), posture) }

            div {
                style: full_width(posture),
                AtCheckRow {
                    checked: break_before,
                    min_touch_px: posture.min_touch_px,
                    aria_label: fl!("style-dialog-flow-break-before"),
                    label: rsx! { { fl!("style-dialog-flow-break-before") } },
                    on_toggle: move |v: bool| {
                        set_flag(draft, v, |d, v| d.style.para_props.page_break_before = Some(v));
                    },
                }
                AtCheckRow {
                    checked: break_after,
                    min_touch_px: posture.min_touch_px,
                    aria_label: fl!("style-dialog-flow-break-after"),
                    label: rsx! { { fl!("style-dialog-flow-break-after") } },
                    on_toggle: move |v: bool| {
                        set_flag(draft, v, |d, v| d.style.para_props.page_break_after = Some(v));
                    },
                }
            }

            { section_heading(fl!("style-dialog-flow-keep-heading"), posture) }

            div {
                style: full_width(posture),
                AtCheckRow {
                    checked: keep_together,
                    min_touch_px: posture.min_touch_px,
                    aria_label: fl!("style-dialog-flow-keep-together"),
                    label: rsx! { { fl!("style-dialog-flow-keep-together") } },
                    on_toggle: move |v: bool| {
                        set_flag(draft, v, |d, v| d.style.para_props.keep_together = Some(v));
                    },
                }
                AtCheckRow {
                    checked: keep_with_next,
                    min_touch_px: posture.min_touch_px,
                    aria_label: fl!("style-dialog-flow-keep-with-next"),
                    label: rsx! { { fl!("style-dialog-flow-keep-with-next") } },
                    on_toggle: move |v: bool| {
                        set_flag(draft, v, |d, v| d.style.para_props.keep_with_next = Some(v));
                    },
                }
            }

            // Orphan/widow counts stay enabled while "do not split" is on: the
            // model keeps them independently, and disabling them here would
            // discard the value the user set for when they turn splitting back
            // on. The notice states the interaction instead.
            { numeric_field(
                fl!("style-dialog-flow-orphans"), catalog, id, draft, open_style, posture,
                |s| s.para_props.orphan_control,
                |n: &u8| fl!("style-dialog-unit-lines-value", value = i64::from(*n)),
                |d| d.buffers.orphan.clone(),
                |d, v| {
                    if let Ok(n) = parse_lines(&v) { d.style.para_props.orphan_control = n; }
                    d.buffers.orphan = v;
                },
                |d| { d.style.para_props.orphan_control = None; d.buffers.orphan = String::new(); },
                lines(), String::new(),
            ) }

            { numeric_field(
                fl!("style-dialog-flow-widows"), catalog, id, draft, open_style, posture,
                |s| s.para_props.widow_control,
                |n: &u8| fl!("style-dialog-unit-lines-value", value = i64::from(*n)),
                |d| d.buffers.widow.clone(),
                |d, v| {
                    if let Ok(n) = parse_lines(&v) { d.style.para_props.widow_control = n; }
                    d.buffers.widow = v;
                },
                |d| { d.style.para_props.widow_control = None; d.buffers.widow = String::new(); },
                lines(), String::new(),
            ) }

            if keep_together {
                AtDialogNotice {
                    tone: AtNoticeTone::Info,
                    extra_style: full_width(posture),
                    message: rsx! { { fl!("style-dialog-flow-keep-together-note") } },
                }
            }

            // Hyphenation has no representation in `ParaProps` at all — not the
            // automatic flag, nor the character counts. It is named rather than
            // drawn: a hyphenation panel that changed nothing would be a defect,
            // and the design's own rule is that an inapplicable control states
            // its reason.
            // TODO(para-props-hyphenation): add `hyphenate`, `hyphen_chars_before`,
            // `hyphen_chars_after` and `hyphen_consecutive_lines` to `ParaProps`,
            // then wire them through the ODF/OOXML mappers and `loki-layout`.
            AtDialogNotice {
                tone: AtNoticeTone::Info,
                extra_style: full_width(posture),
                message: rsx! { { fl!("style-dialog-flow-hyphenation-unsupported") } },
            }
        }
    }
}

/// Applies a boolean edit to the draft.
fn set_flag(mut draft: DraftSignal, value: bool, set: impl Fn(&mut ParaDialogDraft, bool)) {
    let mut next = draft.read().clone();
    if let Some(d) = next.as_mut() {
        set(d, value);
    }
    draft.set(next);
}

/// A full-width uppercase section heading inside the body grid.
fn section_heading(text: String, posture: DialogPosture) -> Element {
    rsx! {
        div {
            style: format!(
                "{span} font-size: {fs}px; font-weight: {fw}; letter-spacing: 0.04em; \
                 text-transform: uppercase; color: {fg};",
                span = full_width(posture),
                fs = tokens::FONT_SIZE_LABEL,
                fw = tokens::FONT_WEIGHT_SEMIBOLD,
                fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
            ),
            {text}
        }
    }
}
