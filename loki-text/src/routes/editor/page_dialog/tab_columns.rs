// SPDX-License-Identifier: Apache-2.0

//! The **Columns** tab: count, gap, and the separator rule.

use appthere_ui::{AtCheckRow, AtDialogNotice, AtField, AtNoticeTone, DialogPosture, tokens};
use dioxus::prelude::*;
use loki_doc_model::layout::page::{PageLayout, SectionColumns};
use loki_doc_model::loki_primitives::units::MeasurementUnit;
use loki_doc_model::loki_primitives::units::Points;
use loki_i18n::fl;

use super::super::editor_defaults::PanelSettings;
use super::PageDialogDraft;
use super::body::{PageDraft, grid_style, measure_field, span_all};

/// The most columns the stepper offers — the model's own ceiling (§3c).
const MAX_COLUMNS: u8 = 12;

/// The gap a newly-columned page starts with (0.5 in), matching the Layout
/// ribbon so a page columned either way looks the same.
const DEFAULT_GAP_PT: f64 = 36.0;

/// Renders the Columns tab body.
pub(super) fn body(draft: PageDraft, posture: DialogPosture, settings: &PanelSettings) -> Element {
    let unit = settings.unit;
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let count = current
        .layout
        .columns
        .as_ref()
        .map_or(1, |c| c.count)
        .max(1);
    let separator = current.layout.columns.as_ref().is_some_and(|c| c.separator);
    let single = count <= 1;

    rsx! {
        div {
            style: grid_style(posture),

            // ── Count (§3c): a stepper over the model's full 1..=12 range ────
            AtField {
                label: fl!("page-dialog-columns-layout"),
                extra_style: span_all(posture),
                control: rsx! {
                    div {
                        style: format!(
                            "display: flex; flex-direction: row; align-items: center; gap: {g}px;",
                            g = tokens::SPACE_2,
                        ),
                        { step_button(fl!("page-dialog-columns-fewer"), count > 1, posture, move |()| {
                            set_count(draft, count - 1, unit);
                        }) }
                        span {
                            style: format!(
                                "font-size: {fs}px; color: {fg}; min-width: 80px; text-align: center;",
                                fs = tokens::FONT_SIZE_BODY,
                                fg = tokens::COLOR_TEXT_ON_CHROME,
                            ),
                            { fl!("page-dialog-columns-count", count = i64::from(count)) }
                        }
                        { step_button(fl!("page-dialog-columns-more"), count < MAX_COLUMNS, posture, move |()| {
                            set_count(draft, count + 1, unit);
                        }) }
                    }
                },
            }

            // Spacing and the rule are properties *between* columns, so with one
            // column they describe nothing. Disabled with the reason rather than
            // hidden — a control that vanishes cannot be learnt.
            { measure_field(
                fl!("page-dialog-columns-gap"), draft, posture, settings,
                |d| d.buffers.column_gap.clone(),
                |d, pt| {
                    if let Some(cols) = d.layout.columns.as_mut() {
                        cols.gap = pt;
                    }
                },
                |d, v| d.buffers.column_gap = v,
                if single { Some(fl!("page-dialog-columns-single")) } else { None },
                single,
                String::new(),
            ) }

            div {
                style: span_all(posture),
                AtCheckRow {
                    checked: separator,
                    disabled: single,
                    min_touch_px: posture.min_touch_px,
                    aria_label: fl!("page-dialog-columns-separator"),
                    label: rsx! { { fl!("page-dialog-columns-separator") } },
                    on_toggle: move |v: bool| {
                        let mut draft = draft;
                        let mut next = draft.read().clone();
                        if let Some(d) = next.as_mut()
                            && let Some(cols) = d.layout.columns.as_mut()
                        {
                            cols.separator = v;
                        }
                        draft.set(next);
                    },
                }
            }

            // ── Per-column widths (§3c) ──────────────────────────────────────
            { super::tab_columns_widths::widths_block(draft, posture, settings, &current) }

            AtDialogNotice {
                tone: AtNoticeTone::Info,
                extra_style: span_all(posture),
                message: rsx! { { fl!("page-dialog-columns-note") } },
            }
        }
    }
}

/// One stepper arm.
fn step_button(
    label: String,
    enabled: bool,
    posture: DialogPosture,
    onclick: impl FnMut(()) + 'static,
) -> Element {
    let mut onclick = onclick;
    rsx! {
        button {
            style: format!(
                "padding: {p}px {p2}px; min-height: {t}px; min-width: {t}px; \
                 border-radius: 3px; cursor: pointer; font-size: {fs}px; \
                 border: 1px solid {border}; background: {bg}; color: {fg};",
                p = tokens::SPACE_1,
                p2 = tokens::SPACE_2,
                t = posture.min_touch_px,
                fs = tokens::FONT_SIZE_BODY,
                border = tokens::COLOR_BORDER_CHROME,
                bg = tokens::COLOR_SURFACE_2,
                fg = tokens::COLOR_TEXT_ON_CHROME,
            ),
            disabled: !enabled,
            aria_label: label.clone(),
            onclick: move |_| onclick(()),
            { label.clone() }
        }
    }
}

/// Sets the column count, adding or clearing the column block as needed.
///
/// One column is the *absence* of columns in the model, not a `SectionColumns`
/// with `count: 1` — writing the latter would export a single-column section
/// as an explicit one-column layout, which is a different document.
/// The column count `layout` currently describes; one when it has no columns.
fn current_count(layout: &PageLayout) -> u8 {
    layout.columns.as_ref().map_or(1, |c| c.count)
}

fn set_count(mut draft: PageDraft, count: u8, unit: MeasurementUnit) {
    let mut next = draft.read().clone();
    if let Some(d) = next.as_mut() {
        if current_count(&d.layout) == count {
            return;
        }
        if count <= 1 {
            d.layout.columns = None;
        } else {
            let gap = d
                .layout
                .columns
                .as_ref()
                .map_or_else(|| Points::new(DEFAULT_GAP_PT), |c| c.gap);
            let separator = d.layout.columns.as_ref().is_some_and(|c| c.separator);
            // §3c: explicit widths survive a count change — truncate on
            // decrease, append the average on increase (the old handler
            // rebuilt with `Vec::new()`, silently discarding imported
            // unequal columns). Empty stays empty: equal columns re-derive.
            let mut widths = d
                .layout
                .columns
                .as_ref()
                .map(|c| c.widths.clone())
                .unwrap_or_default();
            if !widths.is_empty() {
                let avg = Points::new(
                    widths.iter().map(|w| w.value()).sum::<f64>() / widths.len() as f64,
                );
                widths.resize(usize::from(count), avg);
            }
            d.layout.columns = Some(SectionColumns {
                count,
                gap,
                separator,
                widths,
            });
        }
        d.buffers = PageDialogDraft::buffers_for(&d.layout, unit);
    }
    draft.set(next);
}
