// SPDX-License-Identifier: Apache-2.0

//! The **Margins** tab: the four edges, mirroring, and the binding gutter.

use appthere_ui::{AtCheckRow, AtDialogNotice, AtNoticeTone, DialogPosture};
use dioxus::prelude::*;
use loki_doc_model::layout::page::PageUsage;
use loki_doc_model::loki_primitives::units::Points;
use loki_i18n::fl;

use super::super::editor_defaults::PanelSettings;
use super::body::{PageDraft, grid_style, measure_field, section_heading, span_all};
use super::tabs::{
    end_margin_label, is_mirrored, margins_are_equal, start_margin_label, unit_label,
};

/// Renders the Margins tab body.
pub(super) fn body(draft: PageDraft, posture: DialogPosture, settings: &PanelSettings) -> Element {
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let mirrored = is_mirrored(&current.layout);
    let equal = margins_are_equal(&current.layout);
    let text_area = text_area_line(&current, settings);

    rsx! {
        div {
            style: grid_style(posture),

            { section_heading(fl!("page-dialog-margins-heading"), posture) }

            { measure_field(
                fl!("page-dialog-margin-top"), draft, posture, settings,
                |d| d.buffers.margin_top.clone(),
                |d, pt| d.layout.margins.top = pt,
                |d, v| d.buffers.margin_top = v,
                None, false, String::new(),
            ) }
            { measure_field(
                fl!("page-dialog-margin-bottom"), draft, posture, settings,
                |d| d.buffers.margin_bottom.clone(),
                |d, pt| d.layout.margins.bottom = pt,
                |d, v| d.buffers.margin_bottom = v,
                None, false, String::new(),
            ) }
            // Note 14: Inner/Outer while mirroring is on, Left/Right when it is
            // off — the label follows the model, not the screen edge.
            { measure_field(
                start_margin_label(mirrored), draft, posture, settings,
                |d| d.buffers.margin_start.clone(),
                |d, pt| d.layout.margins.left = pt,
                |d, v| d.buffers.margin_start = v,
                None, false, String::new(),
            ) }
            { measure_field(
                end_margin_label(mirrored), draft, posture, settings,
                |d| d.buffers.margin_end.clone(),
                |d, pt| d.layout.margins.right = pt,
                |d, v| d.buffers.margin_end = v,
                None, false, String::new(),
            ) }

            div {
                style: span_all(posture),
                AtCheckRow {
                    checked: mirrored,
                    min_touch_px: posture.min_touch_px,
                    aria_label: fl!("page-dialog-mirror"),
                    label: rsx! { { fl!("page-dialog-mirror") } },
                    on_toggle: move |v: bool| {
                        let mut draft = draft;
                        let mut next = draft.read().clone();
                        if let Some(d) = next.as_mut() {
                            // `Left`/`Right` restrict which pages a layout is
                            // used for; turning mirroring off returns to `All`
                            // rather than picking one of them.
                            d.layout.page_usage =
                                if v { PageUsage::Mirrored } else { PageUsage::All };
                        }
                        draft.set(next);
                    },
                }
            }

            { section_heading(fl!("page-dialog-gutter-heading"), posture) }

            { measure_field(
                fl!("page-dialog-gutter"), draft, posture, settings,
                |d| d.buffers.gutter.clone(),
                |d, pt| d.layout.margins.gutter = pt,
                |d, v| d.buffers.gutter = v,
                Some(fl!("page-dialog-gutter-note")),
                false, String::new(),
            ) }

            if equal {
                AtDialogNotice {
                    tone: AtNoticeTone::Info,
                    extra_style: span_all(posture),
                    // Note 13: equality is decided in points, so this line does
                    // not flip as the display unit changes.
                    message: rsx! { { fl!("page-dialog-margins-equal") } },
                }
            }

            AtDialogNotice {
                tone: AtNoticeTone::Caution,
                extra_style: span_all(posture),
                message: rsx! { {text_area} },
            }
        }
    }
}

/// The resulting text area, stated in the display unit.
///
/// The number a margin change is *for*: a user setting a 31.8 mm inner margin is
/// deciding a measure, and the measure is what they cannot see from the four
/// boxes.
fn text_area_line(draft: &super::PageDialogDraft, settings: &PanelSettings) -> String {
    let l = &draft.layout;
    let m = &l.margins;
    let width = Points::new(
        (l.page_size.width.value() - m.left.value() - m.right.value() - m.gutter.value()).max(0.0),
    );
    let height =
        Points::new((l.page_size.height.value() - m.top.value() - m.bottom.value()).max(0.0));
    let u = settings.unit;
    fl!(
        "page-dialog-text-area",
        width = u.format_bare(width),
        height = u.format_bare(height),
        unit = unit_label(u)
    )
}
