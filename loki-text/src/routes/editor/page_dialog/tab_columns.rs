// SPDX-License-Identifier: Apache-2.0

//! The **Columns** tab: count, gap, and the separator rule.

use appthere_ui::{
    AtCheckRow, AtDialogNotice, AtField, AtNoticeTone, AtSegmented, DialogPosture, tokens,
};
use dioxus::prelude::*;
use loki_doc_model::layout::page::{PageLayout, SectionColumns};
use loki_doc_model::loki_primitives::units::MeasurementUnit;
use loki_doc_model::loki_primitives::units::Points;
use loki_i18n::fl;

use super::super::editor_defaults::PanelSettings;
use super::PageDialogDraft;
use super::body::{PageDraft, grid_style, measure_field, span_all};

/// The counts the layout picker offers. Higher counts stay reachable through
/// the model; three is where a page of body text stops being readable.
const PICKER_COUNTS: [u8; 3] = [1, 2, 3];

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

    // Hoisted: a prop value is an expression position, and an `if` is not one.
    let imported_note: Element = if PICKER_COUNTS.contains(&count) {
        rsx! {}
    } else {
        rsx! {
            div {
                style: format!(
                    "font-size: {fs}px; color: {fg};",
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { fl!("page-dialog-columns-imported", count = i64::from(count)) }
            }
        }
    };

    rsx! {
        div {
            style: grid_style(posture),

            AtField {
                label: fl!("page-dialog-columns-layout"),
                extra_style: span_all(posture),
                control: rsx! {
                    AtSegmented {
                        options: PICKER_COUNTS
                            .iter()
                            .map(|n| fl!("page-dialog-columns-count", count = i64::from(*n)))
                            .collect::<Vec<_>>(),
                        selected: PICKER_COUNTS
                            .iter()
                            .position(|n| *n == count)
                            .unwrap_or(usize::MAX),
                        min_touch_px: posture.min_touch_px,
                        on_select: move |idx: usize| {
                            let Some(n) = PICKER_COUNTS.get(idx).copied() else { return };
                            set_count(draft, n, unit);
                        },
                    }
                },
                footnote: imported_note,
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

            AtDialogNotice {
                tone: AtNoticeTone::Info,
                extra_style: span_all(posture),
                message: rsx! { { fl!("page-dialog-columns-note") } },
            }
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
        // A no-op click must stay a no-op. The rebuild below drops `widths`,
        // so re-picking the count a document was imported with would discard
        // its unequal columns without the user changing anything.
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
            d.layout.columns = Some(SectionColumns {
                count,
                gap,
                separator,
                widths: Vec::new(),
            });
        }
        d.buffers = PageDialogDraft::buffers_for(&d.layout, unit);
    }
    draft.set(next);
}
