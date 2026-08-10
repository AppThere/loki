// SPDX-License-Identifier: Apache-2.0

//! The **Indents & spacing** tab.

use appthere_ui::tokens;
use appthere_ui::{AtCheckRow, AtDialogNotice, AtNoticeTone, DialogPosture};
use dioxus::prelude::*;
use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::style::props::para_props::{LineHeight, Spacing};
use loki_doc_model::style::{StyleCatalog, StyleId};
use loki_i18n::fl;

use super::draft::{fmt_points, parse_multiple, parse_points, trim_float};
use super::fields::{DraftSignal, OpenSignal, body_grid_style, full_width, numeric_field};

/// Renders the Indents & spacing tab body.
pub(super) fn body(
    catalog: &StyleCatalog,
    id: &StyleId,
    draft: DraftSignal,
    open_style: OpenSignal,
    posture: DialogPosture,
) -> Element {
    let pt = || Some(fl!("style-dialog-unit-pt-short"));
    let fmt_pt = |p: &Points| fl!("style-dialog-unit-pt", value = fmt_points(Some(*p)));
    let fmt_sp = |s: &Spacing| match s {
        Spacing::Exact(p) => fl!("style-dialog-unit-pt", value = fmt_points(Some(*p))),
        other => format!("{other:?}"),
    };

    rsx! {
        div {
            style: body_grid_style(posture),

            div {
                style: format!(
                    "{span} font-size: {fs}px; font-weight: {fw}; letter-spacing: 0.04em; \
                     text-transform: uppercase; color: {fg};",
                    span = full_width(posture),
                    fs = tokens::FONT_SIZE_LABEL,
                    fw = tokens::FONT_WEIGHT_SEMIBOLD,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { fl!("style-dialog-indents-heading") }
            }

            { numeric_field(
                fl!("style-dialog-indents-before"), catalog, id, draft, open_style, posture,
                |s| s.para_props.indent_start, fmt_pt,
                |d| d.buffers.indent_start.clone(),
                |d, v| {
                    if let Ok(p) = parse_points(&v) { d.style.para_props.indent_start = p; }
                    d.buffers.indent_start = v;
                },
                |d| { d.style.para_props.indent_start = None; d.buffers.indent_start = String::new(); },
                pt(), String::new(),
            ) }

            { numeric_field(
                fl!("style-dialog-indents-after"), catalog, id, draft, open_style, posture,
                |s| s.para_props.indent_end, fmt_pt,
                |d| d.buffers.indent_end.clone(),
                |d, v| {
                    if let Ok(p) = parse_points(&v) { d.style.para_props.indent_end = p; }
                    d.buffers.indent_end = v;
                },
                |d| { d.style.para_props.indent_end = None; d.buffers.indent_end = String::new(); },
                pt(), String::new(),
            ) }

            { numeric_field(
                fl!("style-dialog-indents-first-line"), catalog, id, draft, open_style, posture,
                |s| s.para_props.indent_first_line, fmt_pt,
                |d| d.buffers.indent_first.clone(),
                |d, v| {
                    if let Ok(p) = parse_points(&v) { d.style.para_props.indent_first_line = p; }
                    d.buffers.indent_first = v;
                },
                |d| { d.style.para_props.indent_first_line = None; d.buffers.indent_first = String::new(); },
                pt(), String::new(),
            ) }

            div {
                style: format!(
                    "{span} margin-top: {mt}px; font-size: {fs}px; font-weight: {fw}; \
                     letter-spacing: 0.04em; text-transform: uppercase; color: {fg};",
                    span = full_width(posture),
                    mt = tokens::SPACE_2,
                    fs = tokens::FONT_SIZE_LABEL,
                    fw = tokens::FONT_WEIGHT_SEMIBOLD,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { fl!("style-dialog-spacing-heading") }
            }

            { numeric_field(
                fl!("style-dialog-spacing-above"), catalog, id, draft, open_style, posture,
                |s| s.para_props.space_before, fmt_sp,
                |d| d.buffers.space_before.clone(),
                |d, v| {
                    if let Ok(p) = parse_points(&v) {
                        d.style.para_props.space_before = p.map(Spacing::Exact);
                    }
                    d.buffers.space_before = v;
                },
                |d| { d.style.para_props.space_before = None; d.buffers.space_before = String::new(); },
                pt(), String::new(),
            ) }

            { numeric_field(
                fl!("style-dialog-spacing-below"), catalog, id, draft, open_style, posture,
                |s| s.para_props.space_after, fmt_sp,
                |d| d.buffers.space_after.clone(),
                |d, v| {
                    if let Ok(p) = parse_points(&v) {
                        d.style.para_props.space_after = p.map(Spacing::Exact);
                    }
                    d.buffers.space_after = v;
                },
                |d| { d.style.para_props.space_after = None; d.buffers.space_after = String::new(); },
                pt(), String::new(),
            ) }

            // Line height is a *multiple*, not a length — 1.35 means 1.35×,
            // matching the layout engine and the OOXML mapper. The unit suffix
            // says so, because a box reading `1.35` with a `pt` beside it would
            // be read as a 1.35 pt leading.
            { numeric_field(
                fl!("style-dialog-spacing-line-height"), catalog, id, draft, open_style, posture,
                |s| s.para_props.line_height,
                |lh: &LineHeight| match lh {
                    LineHeight::Multiple(m) => fl!(
                        "style-dialog-unit-multiple",
                        value = trim_float(f64::from(*m))
                    ),
                    LineHeight::Exact(p) => fl!("style-dialog-unit-pt", value = fmt_points(Some(*p))),
                    LineHeight::AtLeast(p) => fl!(
                        "style-dialog-unit-pt-at-least",
                        value = fmt_points(Some(*p))
                    ),
                    other => format!("{other:?}"),
                },
                |d| d.buffers.line_height.clone(),
                |d, v| {
                    if let Ok(m) = parse_multiple(&v) {
                        d.style.para_props.line_height = m.map(LineHeight::Multiple);
                    }
                    d.buffers.line_height = v;
                },
                |d| { d.style.para_props.line_height = None; d.buffers.line_height = String::new(); },
                Some(fl!("style-dialog-unit-multiple-short")),
                String::new(),
            ) }

            // Contextual spacing has no field in `ParaProps`, so the control is
            // shown disabled with its reason rather than hidden — a checkbox
            // that silently did nothing would be worse than an absent one.
            // TODO(para-props-contextual-spacing): add `contextual_spacing` to
            // `ParaProps` and wire this through the ODF/OOXML mappers.
            div {
                style: full_width(posture),
                AtCheckRow {
                    checked: false,
                    disabled: true,
                    min_touch_px: posture.min_touch_px,
                    aria_label: fl!("style-dialog-spacing-collapse"),
                    label: rsx! { { fl!("style-dialog-spacing-collapse") } },
                    on_toggle: move |_| {},
                }
            }
            AtDialogNotice {
                tone: AtNoticeTone::Info,
                extra_style: full_width(posture),
                message: rsx! { { fl!("style-dialog-spacing-collapse-unsupported") } },
            }
        }
    }
}
