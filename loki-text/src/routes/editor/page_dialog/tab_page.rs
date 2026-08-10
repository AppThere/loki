// SPDX-License-Identifier: Apache-2.0

//! The **Page** tab: paper, orientation, and the resulting size.

use std::sync::{Arc, Mutex};

use appthere_ui::{AtDialogNotice, AtField, AtNoticeTone, AtSegmented, DialogPosture, tokens};
use dioxus::prelude::*;
use loki_doc_model::layout::page::{PageOrientation, PageSize};
use loki_doc_model::layout::paper_catalog::{self, Paper};
use loki_doc_model::loki_primitives::units::MeasurementUnit;
use loki_i18n::fl;

use super::super::editor_defaults::PanelSettings;
use super::PageDialogDraft;
use super::body::{PageDraft, grid_style, measure_field, span_all};
use super::tabs::PaperOrigin;
use crate::editing::state::DocumentState;

/// Renders the Page tab body.
pub(super) fn body(
    doc_state: &Arc<Mutex<DocumentState>>,
    draft: PageDraft,
    posture: DialogPosture,
    settings: &PanelSettings,
) -> Element {
    let unit = settings.unit;
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    // An explicit Custom choice wins over a size that still matches a paper.
    let origin = if current.custom_paper {
        PaperOrigin::Custom
    } else {
        PaperOrigin::of(&current.layout)
    };
    let is_landscape =
        current.layout.page_size.width.value() > current.layout.page_size.height.value();
    let origin_line = origin.line(&current.layout, settings.unit);
    let sections = sections_using(doc_state, &current.name);

    rsx! {
        div {
            style: grid_style(posture),

            // ── Paper ─────────────────────────────────────────────────────────
            AtField {
                label: fl!("page-dialog-paper"),
                control: rsx! {
                    div {
                        style: format!(
                            "display: flex; flex-direction: row; flex-wrap: wrap; gap: {gap}px;",
                            gap = tokens::SPACE_2,
                        ),
                        for paper in paper_catalog::PAPERS.iter() {
                            button {
                                key: "{paper.id}",
                                style: paper_button_style(
                                    paper.matches(&current.layout.page_size),
                                    posture,
                                ),
                                onclick: move |evt| {
                                    evt.stop_propagation();
                                    set_paper(draft, paper, unit);
                                },
                                {paper.display_name}
                            }
                        }
                        // Without this chip the size boxes were unreachable: a
                        // catalogued size locked them, and the only documented
                        // way out was to edit them.
                        button {
                            key: "custom",
                            style: paper_button_style(!origin.is_preset(), posture),
                            onclick: move |evt| {
                                evt.stop_propagation();
                                set_custom(draft);
                            },
                            { fl!("page-dialog-paper-custom") }
                        }
                    }
                },
                footnote: rsx! {
                    div {
                        style: format!(
                            "display: flex; align-items: center; gap: {gap}px; \
                             font-size: {fs}px; color: {fg};",
                            gap = tokens::SPACE_1,
                            fs = tokens::FONT_SIZE_LABEL,
                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        ),
                        span { "\u{21B3}" }
                        span { {origin_line} }
                    }
                },
            }

            // ── Orientation ───────────────────────────────────────────────────
            AtField {
                label: fl!("page-dialog-orientation"),
                control: rsx! {
                    AtSegmented {
                        options: vec![
                            fl!("page-dialog-portrait"),
                            fl!("page-dialog-landscape"),
                        ],
                        selected: usize::from(is_landscape),
                        min_touch_px: posture.min_touch_px,
                        on_select: move |idx: usize| {
                            set_orientation(
                                draft,
                                if idx == 1 {
                                    PageOrientation::Landscape
                                } else {
                                    PageOrientation::Portrait
                                },
                                unit,
                            );
                        },
                    }
                },
            }

            // ── Width × height ────────────────────────────────────────────────
            // Note 12: read-only under a named preset. Editing them is how a
            // user reaches Custom, so the boxes unlock the moment the size stops
            // matching a catalogued paper rather than staying locked forever.
            { measure_field(
                fl!("page-dialog-width"), draft, posture, settings,
                |d| d.buffers.width.clone(),
                |d, pt| d.layout.page_size.width = pt,
                |d, v| d.buffers.width = v,
                None,
                origin.is_preset(),
                String::new(),
            ) }
            { measure_field(
                fl!("page-dialog-height"), draft, posture, settings,
                |d| d.buffers.height.clone(),
                |d, pt| d.layout.page_size.height = pt,
                |d, v| d.buffers.height = v,
                None,
                origin.is_preset(),
                String::new(),
            ) }

            if origin.is_preset() {
                AtDialogNotice {
                    tone: AtNoticeTone::Info,
                    extra_style: span_all(posture),
                    message: rsx! { { fl!("page-dialog-preset-locked") } },
                }
            }

            // ── Impact ────────────────────────────────────────────────────────
            // Note 11: page styles do not inherit, so the consequence worth
            // stating is not "what changes downstream" but "which sections use
            // this geometry" — the only thing this edit can reach.
            AtDialogNotice {
                tone: if sections == 0 { AtNoticeTone::Caution } else { AtNoticeTone::Info },
                extra_style: span_all(posture),
                message: rsx! {
                    { fl!("page-dialog-impact", count = sections as i64) }
                },
            }
        }
    }
}

/// How many sections reference the page style named `name`.
fn sections_using(doc_state: &Arc<Mutex<DocumentState>>, name: &str) -> usize {
    let Ok(state) = doc_state.lock() else {
        return 0;
    };
    let Some(doc) = state.document.as_ref() else {
        return 0;
    };
    doc.sections
        .iter()
        .filter(|s| s.page_style.as_ref().is_some_and(|p| p.as_str() == name))
        .count()
}

/// Applies a catalogued paper, keeping the current orientation.
fn set_paper(mut draft: PageDraft, paper: &'static Paper, unit: MeasurementUnit) {
    let mut next = draft.read().clone();
    if let Some(d) = next.as_mut() {
        d.layout.page_size = paper.oriented_like(&d.layout.page_size);
        d.custom_paper = false;
        d.buffers = PageDialogDraft::buffers_for(&d.layout, unit);
    }
    draft.set(next);
}

/// Unlocks the width and height boxes by declaring the size custom.
///
/// The size itself is untouched — the user is saying "let me change this", not
/// "change it for me" — so the boxes open on the numbers already there.
fn set_custom(mut draft: PageDraft) {
    let mut next = draft.read().clone();
    if let Some(d) = next.as_mut() {
        d.custom_paper = true;
    }
    draft.set(next);
}

/// Swaps the axes when the requested orientation differs from the current one.
fn set_orientation(mut draft: PageDraft, want: PageOrientation, unit: MeasurementUnit) {
    let mut next = draft.read().clone();
    if let Some(d) = next.as_mut() {
        let size = &d.layout.page_size;
        let is_landscape = size.width.value() > size.height.value();
        let want_landscape = want == PageOrientation::Landscape;
        if is_landscape != want_landscape {
            d.layout.page_size = PageSize {
                width: size.height,
                height: size.width,
            };
        }
        d.layout.orientation = want;
        d.buffers = PageDialogDraft::buffers_for(&d.layout, unit);
    }
    draft.set(next);
}

/// A paper-choice button.
fn paper_button_style(active: bool, posture: DialogPosture) -> String {
    format!(
        "{touch} box-sizing: border-box; padding: {py}px {px}px; border-radius: {r}px; \
         cursor: pointer; white-space: nowrap; font-size: {fs}px; \
         background: {bg}; border: 1px solid {border}; color: {fg};",
        touch = if posture.min_touch_px > 0.0 {
            format!("min-height: {}px;", posture.min_touch_px)
        } else {
            String::new()
        },
        py = tokens::SPACE_2,
        px = tokens::SPACE_3,
        r = tokens::RADIUS_MD,
        fs = tokens::FONT_SIZE_BODY,
        bg = if active {
            tokens::COLOR_SURFACE_3
        } else {
            tokens::COLOR_SURFACE_2
        },
        border = if active {
            tokens::COLOR_TAB_ACTIVE_INDICATOR
        } else {
            tokens::COLOR_BORDER_CHROME
        },
        fg = if active {
            tokens::COLOR_TEXT_ON_CHROME
        } else {
            tokens::COLOR_TEXT_ON_CHROME_SECONDARY
        },
    )
}
