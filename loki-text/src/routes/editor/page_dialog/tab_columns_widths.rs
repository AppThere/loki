// SPDX-License-Identifier: Apache-2.0

//! The Columns tab's **per-column widths** block (§3c): switch between equal
//! columns (the model's empty `widths`) and explicit per-column widths, with
//! one measurement field per column. The layout honours explicit widths only
//! when one is present per column, which the materialize/truncate logic here
//! maintains.

use appthere_ui::{AtField, DialogPosture, tokens};
use dioxus::prelude::*;
use loki_doc_model::loki_primitives::units::Points;
use loki_i18n::fl;

use super::super::editor_defaults::PanelSettings;
use super::PageDialogDraft;
use super::body::{PageDraft, measure_field, span_all};

/// Renders the widths block: nothing for a single-column page; an enable
/// button while columns are equal; the per-column fields plus an
/// "equal columns" reset once explicit widths exist.
pub(super) fn widths_block(
    draft: PageDraft,
    posture: DialogPosture,
    settings: &PanelSettings,
    current: &PageDialogDraft,
) -> Element {
    let Some(cols) = current.layout.columns.as_ref() else {
        return rsx! {};
    };
    if cols.count <= 1 {
        return rsx! {};
    }
    let unit = settings.unit;
    let count = cols.count;

    if cols.widths.is_empty() {
        // Equal columns: offer the switch to explicit widths, seeded with the
        // equal split so the fields open showing the numbers the page already
        // uses.
        return rsx! {
            div {
                style: span_all(posture),
                { action_button(fl!("page-dialog-columns-custom-widths"), posture, move |()| {
                    materialize_equal_widths(draft, unit);
                }) }
            }
        };
    }

    rsx! {
        AtField {
            label: fl!("page-dialog-columns-widths"),
            extra_style: span_all(posture),
            control: rsx! {
                div {
                    style: format!(
                        "display: flex; flex-direction: row; flex-wrap: wrap; gap: {g}px; \
                         align-items: flex-end;",
                        g = tokens::SPACE_2,
                    ),
                    for idx in 0..usize::from(count) {
                        { width_field(draft, posture, settings, idx) }
                    }
                    { action_button(fl!("page-dialog-columns-equal"), posture, move |()| {
                        clear_widths(draft, unit);
                    }) }
                }
            },
        }
    }
}

/// One column's width field, bound to `buffers.column_widths[idx]`.
fn width_field(
    draft: PageDraft,
    posture: DialogPosture,
    settings: &PanelSettings,
    idx: usize,
) -> Element {
    measure_field(
        fl!("page-dialog-columns-width-n", n = (idx + 1) as i64),
        draft,
        posture,
        settings,
        move |d| {
            d.buffers
                .column_widths
                .get(idx)
                .cloned()
                .unwrap_or_default()
        },
        move |d, pt| {
            if let Some(cols) = d.layout.columns.as_mut()
                && let Some(w) = cols.widths.get_mut(idx)
            {
                *w = pt;
            }
        },
        move |d, v| {
            if let Some(slot) = d.buffers.column_widths.get_mut(idx) {
                *slot = v;
            }
        },
        None,
        false,
        String::new(),
    )
}

/// Seeds explicit widths with the equal split the layout would derive: the
/// content width (page minus side margins and gutter) minus the gaps, split
/// over the count.
fn materialize_equal_widths(
    mut draft: PageDraft,
    unit: loki_doc_model::loki_primitives::units::MeasurementUnit,
) {
    let mut next = draft.read().clone();
    if let Some(d) = next.as_mut() {
        let content = d.layout.page_size.width.value()
            - d.layout.margins.left.value()
            - d.layout.margins.right.value()
            - d.layout.margins.gutter.value();
        if let Some(cols) = d.layout.columns.as_mut() {
            let n = f64::from(cols.count);
            let share = ((content - (n - 1.0) * cols.gap.value()) / n).max(0.0);
            cols.widths = vec![Points::new(share); usize::from(cols.count)];
        }
        d.buffers = PageDialogDraft::buffers_for(&d.layout, unit);
    }
    draft.set(next);
}

/// Back to equal columns: the model's empty `widths`.
fn clear_widths(
    mut draft: PageDraft,
    unit: loki_doc_model::loki_primitives::units::MeasurementUnit,
) {
    let mut next = draft.read().clone();
    if let Some(d) = next.as_mut() {
        if let Some(cols) = d.layout.columns.as_mut() {
            cols.widths = Vec::new();
        }
        d.buffers = PageDialogDraft::buffers_for(&d.layout, unit);
    }
    draft.set(next);
}

/// A small action button at the dialog posture's touch minimum.
fn action_button(
    label: String,
    posture: DialogPosture,
    onclick: impl FnMut(()) + 'static,
) -> Element {
    let mut onclick = onclick;
    rsx! {
        button {
            style: format!(
                "padding: {p}px {p2}px; min-height: {t}px; border-radius: 3px; \
                 cursor: pointer; font-size: {fs}px; border: 1px solid {border}; \
                 background: {bg}; color: {fg};",
                p = tokens::SPACE_1,
                p2 = tokens::SPACE_2,
                t = posture.min_touch_px,
                fs = tokens::FONT_SIZE_LABEL,
                border = tokens::COLOR_BORDER_CHROME,
                bg = tokens::COLOR_SURFACE_2,
                fg = tokens::COLOR_TEXT_ON_CHROME,
            ),
            onclick: move |_| onclick(()),
            { label }
        }
    }
}
