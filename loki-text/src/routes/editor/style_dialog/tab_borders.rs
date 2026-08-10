// SPDX-License-Identifier: Apache-2.0

//! The **Borders** tab: paragraph edges and their padding.

use appthere_ui::tokens;
use appthere_ui::{
    AtDialogNotice, AtField, AtNoticeTone, AtSegmented, DialogPosture, at_control_style,
};
use dioxus::prelude::*;
use loki_doc_model::loki_primitives::color::DocumentColor;
use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::style::props::border::BorderStyle;
use loki_doc_model::style::{StyleCatalog, StyleId};
use loki_i18n::fl;

use super::borders::{BORDER_STYLES, BorderEdges, edge_border};
use super::draft::{fmt_points, parse_points};
use super::fields::{DraftSignal, OpenSignal, body_grid_style, full_width, numeric_field};
use super::tab_borders_fields::{default_border, padding_field, restyle, section_heading};

/// The localized label for an edge preset.
fn edge_label(e: BorderEdges) -> String {
    match e {
        BorderEdges::None => fl!("style-dialog-borders-none"),
        BorderEdges::StartOnly => fl!("style-dialog-borders-start-only"),
        BorderEdges::All => fl!("style-dialog-borders-all"),
        BorderEdges::Mixed => fl!("style-dialog-borders-mixed"),
    }
}

/// The localized label for a border line style.
fn style_label(s: BorderStyle) -> String {
    match s {
        BorderStyle::Dashed => fl!("style-dialog-borders-dashed"),
        BorderStyle::Dotted => fl!("style-dialog-borders-dotted"),
        BorderStyle::Double => fl!("style-dialog-borders-double"),
        BorderStyle::Wave => fl!("style-dialog-borders-wave"),
        _ => fl!("style-dialog-borders-solid"),
    }
}

/// Renders the Borders tab body.
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
    let style = current.style.clone();
    let edges = BorderEdges::of(&style);
    let border = edge_border(&style);
    let has_border = border.is_some();
    let line_style = border
        .as_ref()
        .map(|b| b.style)
        .unwrap_or(BorderStyle::Solid);
    let selected_style = BORDER_STYLES
        .iter()
        .position(|s| *s == line_style)
        .unwrap_or(usize::MAX);
    let pt = || Some(fl!("style-dialog-unit-pt-short"));
    let fmt_pt = |p: &Points| fl!("style-dialog-unit-pt", value = fmt_points(Some(*p)));

    // Built before the `rsx!` block: a prop value is an expression position, and
    // an `if` there is not one.
    let no_edge_note: Element = if has_border {
        rsx! {}
    } else {
        rsx! {
            div {
                style: format!(
                    "font-size: {fs}px; color: {fg};",
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                ),
                { fl!("style-dialog-borders-no-edge") }
            }
        }
    };

    rsx! {
        div {
            style: body_grid_style(posture),

            AtField {
                label: fl!("style-dialog-borders-edges"),
                extra_style: full_width(posture),
                control: rsx! {
                    AtSegmented {
                        options: BorderEdges::SELECTABLE
                            .iter()
                            .map(|e| edge_label(*e))
                            .collect::<Vec<_>>(),
                        selected: edges.selectable_index().unwrap_or(usize::MAX),
                        min_touch_px: posture.min_touch_px,
                        on_select: move |idx: usize| {
                            let mut draft = draft;
                            let mut next = draft.read().clone();
                            if let (Some(d), Some(preset)) =
                                (next.as_mut(), BorderEdges::SELECTABLE.get(idx))
                            {
                                let rule = edge_border(&d.style).unwrap_or_else(default_border);
                                preset.apply(&mut d.style, rule.clone());
                                d.buffers.border_width = fmt_points(Some(rule.width));
                            }
                            draft.set(next);
                        },
                    }
                },
            }

            // An imported edge set the presets cannot name is reported, not
            // rounded — selecting a preset is what changes it, deliberately.
            if !edges.is_selectable() {
                AtDialogNotice {
                    tone: AtNoticeTone::Caution,
                    extra_style: full_width(posture),
                    message: rsx! { { fl!("style-dialog-borders-mixed-note") } },
                }
            }

            // The line controls describe the edges that exist. With no border
            // they stay visible and disabled, so the tab's shape does not
            // change as the preset does.
            AtField {
                label: fl!("style-dialog-borders-line-style"),
                disabled: !has_border,
                control: rsx! {
                    AtSegmented {
                        options: BORDER_STYLES.iter().map(|s| style_label(*s)).collect::<Vec<_>>(),
                        selected: selected_style,
                        disabled: !has_border,
                        min_touch_px: posture.min_touch_px,
                        on_select: move |idx: usize| {
                            let mut draft = draft;
                            let mut next = draft.read().clone();
                            if let (Some(d), Some(s)) = (next.as_mut(), BORDER_STYLES.get(idx)) {
                                restyle(d, |b| b.style = *s);
                            }
                            draft.set(next);
                        },
                    }
                },
                footnote: no_edge_note,
            }

            { numeric_field(
                fl!("style-dialog-borders-width"), catalog, id, draft, open_style, posture,
                |s| edge_border(s).map(|b| b.width), fmt_pt,
                |d| d.buffers.border_width.clone(),
                |d, v| {
                    // A zero or negative rule is not a rule; keep the text so
                    // the user can finish typing, but do not commit it.
                    if let Ok(Some(w)) = parse_points(&v)
                        && w.value() > 0.0
                    {
                        restyle(d, |b| b.width = w);
                    }
                    d.buffers.border_width = v;
                },
                |d| {
                    BorderEdges::None.apply(&mut d.style, default_border());
                    d.buffers.border_width = String::new();
                },
                pt(), String::new(),
            ) }

            AtField {
                label: fl!("style-dialog-borders-colour"),
                disabled: !has_border,
                control: rsx! {
                    div {
                        style: at_control_style(posture.min_touch_px, "width: 100%;"),
                        span {
                            style: format!(
                                "flex-shrink: 0; width: 16px; height: 16px; \
                                 border-radius: {r}px; background: {fill}; \
                                 border: 1px solid {b};",
                                r = tokens::RADIUS_SM,
                                fill = border
                                    .as_ref()
                                    .and_then(|b| b.color.as_ref())
                                    .and_then(DocumentColor::to_hex)
                                    .unwrap_or_else(|| tokens::COLOR_ICON_DISABLED.to_string()),
                                b = tokens::COLOR_BORDER_CHROME,
                            ),
                        }
                        span {
                            style: "flex: 1; min-width: 0;",
                            {
                                border
                                    .as_ref()
                                    .and_then(|b| b.color.as_ref())
                                    .and_then(DocumentColor::to_hex)
                                    .unwrap_or_else(|| fl!("style-dialog-borders-colour-automatic"))
                            }
                        }
                    }
                },
            }

            { section_heading(fl!("style-dialog-borders-padding-heading"), posture) }

            { padding_field(
                fl!("style-dialog-borders-padding-top"), catalog, id, draft, open_style, posture,
                |s| s.para_props.padding_top,
                |d| d.buffers.padding_top.clone(),
                |d, v| { if let Ok(p) = parse_points(&v) { d.style.para_props.padding_top = p; } d.buffers.padding_top = v; },
                |d| { d.style.para_props.padding_top = None; d.buffers.padding_top = String::new(); },
            ) }
            { padding_field(
                fl!("style-dialog-borders-padding-bottom"), catalog, id, draft, open_style, posture,
                |s| s.para_props.padding_bottom,
                |d| d.buffers.padding_bottom.clone(),
                |d, v| { if let Ok(p) = parse_points(&v) { d.style.para_props.padding_bottom = p; } d.buffers.padding_bottom = v; },
                |d| { d.style.para_props.padding_bottom = None; d.buffers.padding_bottom = String::new(); },
            ) }
            { padding_field(
                fl!("style-dialog-borders-padding-left"), catalog, id, draft, open_style, posture,
                |s| s.para_props.padding_left,
                |d| d.buffers.padding_left.clone(),
                |d, v| { if let Ok(p) = parse_points(&v) { d.style.para_props.padding_left = p; } d.buffers.padding_left = v; },
                |d| { d.style.para_props.padding_left = None; d.buffers.padding_left = String::new(); },
            ) }
            { padding_field(
                fl!("style-dialog-borders-padding-right"), catalog, id, draft, open_style, posture,
                |s| s.para_props.padding_right,
                |d| d.buffers.padding_right.clone(),
                |d, v| { if let Ok(p) = parse_points(&v) { d.style.para_props.padding_right = p; } d.buffers.padding_right = v; },
                |d| { d.style.para_props.padding_right = None; d.buffers.padding_right = String::new(); },
            ) }
        }
    }
}
