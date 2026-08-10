// SPDX-License-Identifier: Apache-2.0

//! Field and edge helpers for the Borders tab, split from [`super::tab_borders`]
//! to keep both files under the 300-line ceiling.

use appthere_ui::{DialogPosture, tokens};
use dioxus::prelude::*;
use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::style::props::border::{Border, BorderStyle};
use loki_doc_model::style::{StyleCatalog, StyleId};

/// The width a newly-added edge starts at, when the style had no border to
/// inherit one from.
const DEFAULT_BORDER_WIDTH_PT: f64 = 1.0;
use loki_i18n::fl;

use super::borders::{BorderEdges, edge_border};
use super::draft::{ParaDialogDraft, fmt_points};
use super::fields::{DraftSignal, OpenSignal, full_width, numeric_field};

/// A padding measurement field — the four share every argument but their
/// accessor, so they are built by one helper rather than four near-copies.
#[allow(clippy::too_many_arguments)]
pub(super) fn padding_field(
    label: String,
    catalog: &StyleCatalog,
    id: &StyleId,
    draft: DraftSignal,
    open_style: OpenSignal,
    posture: DialogPosture,
    get: impl Fn(&loki_doc_model::style::ParagraphStyle) -> Option<Points> + 'static,
    buffer: impl Fn(&ParaDialogDraft) -> String,
    commit: impl Fn(&mut ParaDialogDraft, String) + 'static,
    reset: impl Fn(&mut ParaDialogDraft) + 'static,
) -> Element {
    numeric_field(
        label,
        catalog,
        id,
        draft,
        open_style,
        posture,
        get,
        |p: &Points| fl!("style-dialog-unit-pt", value = fmt_points(Some(*p))),
        buffer,
        commit,
        reset,
        Some(fl!("style-dialog-unit-pt-short")),
        String::new(),
    )
}

/// The border a newly-added edge starts with.
pub(super) fn default_border() -> Border {
    Border {
        style: BorderStyle::Solid,
        width: Points::new(DEFAULT_BORDER_WIDTH_PT),
        // `None` is "automatic": the edge takes the document's text colour, so
        // a border added in a dark-on-light document is not invisible in a
        // light-on-dark one.
        color: None,
        spacing: None,
    }
}

/// Re-applies the current edge preset with one property of the shared border
/// changed, so every edge that is on stays consistent.
pub(super) fn restyle(d: &mut ParaDialogDraft, change: impl Fn(&mut Border)) {
    let preset = BorderEdges::of(&d.style);
    let Some(mut border) = edge_border(&d.style) else {
        return;
    };
    change(&mut border);
    preset.apply(&mut d.style, border);
}

/// A full-width uppercase section heading inside the body grid.
pub(super) fn section_heading(text: String, posture: DialogPosture) -> Element {
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
