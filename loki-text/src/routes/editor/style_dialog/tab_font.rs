// SPDX-License-Identifier: Apache-2.0

//! The **Font** tab: the style's run-default character properties.

use std::rc::Rc;

use appthere_ui::tokens;
use appthere_ui::{
    AtDialogNotice, AtField, AtNoticeTone, AtSegmented, DialogPosture, at_control_style,
};
use dioxus::prelude::*;
use loki_doc_model::style::{StyleCatalog, StyleId};
use loki_i18n::fl;

use super::draft::{ParaDialogDraft, fmt_points, parse_points};
use super::fields::{
    DraftSignal, OpenSignal, body_grid_style, full_width, numeric_field, provenance_line,
};
use super::font_picker;
use super::rows::{local_property_count, resolve_row};

/// The three weight/posture presets the segmented control offers.
///
/// Weight is a 1–1000 axis in the model, but a paragraph *style*'s run default
/// is almost always one of these three. The full axis stays available on the
/// character style form, which is where a display face's 300 or 800 belongs.
const POSTURES: [(u16, bool); 3] = [(400, false), (700, false), (400, true)];

/// Renders the Font tab body.
pub(super) fn body(
    catalog: &StyleCatalog,
    id: &StyleId,
    draft: DraftSignal,
    open_style: OpenSignal,
    posture: DialogPosture,
    font_families: Rc<Vec<String>>,
) -> Element {
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let style = current.style.clone();
    let cp = style.char_props.clone();

    // Which posture preset the style is in. A weight off the preset axis (say
    // 300) matches none of them, so nothing is highlighted rather than the
    // nearest one being claimed as exact.
    let weight = cp
        .font_weight
        .unwrap_or(if cp.bold == Some(true) { 700 } else { 400 });
    let italic = cp.italic.unwrap_or(false);
    let selected_posture = POSTURES
        .iter()
        .position(|(w, i)| *w == weight && *i == italic)
        .unwrap_or(usize::MAX);

    let weight_row = resolve_row(catalog, id, |s| s.char_props.bold, |b| b.to_string());
    let colour_row = resolve_row(
        catalog,
        id,
        |s| s.char_props.color.clone(),
        |c| {
            c.to_hex()
                .unwrap_or_else(|| fl!("style-dialog-font-colour-automatic"))
        },
    );
    let language_row = resolve_row(
        catalog,
        id,
        |s| s.char_props.language.clone(),
        |l| l.as_str().to_string(),
    );

    let local_count = local_property_count(&style);

    rsx! {
        div {
            style: body_grid_style(posture),

            // ── Font family ───────────────────────────────────────────────────
            { font_picker::field(catalog, id, draft, open_style, posture, font_families) }

            // ── Size ──────────────────────────────────────────────────────────
            { numeric_field(
                fl!("style-dialog-font-size"),
                catalog,
                id,
                draft,
                open_style,
                posture,
                |s| s.char_props.font_size,
                |p| fl!("style-dialog-unit-pt", value = fmt_points(Some(*p))),
                |d| d.buffers.font_size.clone(),
                |d, v| {
                    if let Ok(parsed) = parse_points(&v) {
                        // A zero or negative type size is not a size; keep the
                        // text so the user can finish, but do not commit it.
                        if parsed.is_none_or(|p| p.value() > 0.0) {
                            d.style.char_props.font_size = parsed;
                        }
                    }
                    d.buffers.font_size = v;
                },
                |d| {
                    d.style.char_props.font_size = None;
                    d.buffers.font_size = String::new();
                },
                Some(fl!("style-dialog-unit-pt-short")),
                String::new(),
            ) }

            // ── Weight & posture ──────────────────────────────────────────────
            AtField {
                label: fl!("style-dialog-font-posture"),
                control: rsx! {
                    AtSegmented {
                        options: vec![
                            fl!("style-dialog-font-regular"),
                            fl!("style-dialog-font-bold"),
                            fl!("style-dialog-font-italic"),
                        ],
                        option_styles: vec![
                            String::new(),
                            format!("font-weight: {};", tokens::FONT_WEIGHT_BOLD),
                            "font-style: italic;".to_string(),
                        ],
                        selected: selected_posture,
                        min_touch_px: posture.min_touch_px,
                        on_select: move |idx: usize| {
                            let mut draft = draft;
                            let mut next = draft.read().clone();
                            if let (Some(d), Some((w, i))) =
                                (next.as_mut(), POSTURES.get(idx))
                            {
                                set_posture(d, *w, *i);
                            }
                            draft.set(next);
                        },
                    }
                },
                footnote: weight_row
                    .as_ref()
                    .map(|(source, value)| {
                        provenance_line(
                            source,
                            value.as_deref(),
                            posture,
                            open_style,
                            draft,
                            |d| {
                                d.style.char_props.bold = None;
                                d.style.char_props.font_weight = None;
                                d.style.char_props.italic = None;
                            },
                        )
                    })
                    .unwrap_or_else(|| rsx! {}),
            }

            // ── Text colour ───────────────────────────────────────────────────
            AtField {
                label: fl!("style-dialog-font-colour"),
                control: rsx! {
                    div {
                        style: at_control_style(posture.min_touch_px, "width: 100%;"),
                        span {
                            style: format!(
                                "flex-shrink: 0; width: 16px; height: 16px; \
                                 border-radius: {r}px; background: {fill}; \
                                 border: 1px solid {border};",
                                r = tokens::RADIUS_SM,
                                fill = cp
                                    .color
                                    .as_ref()
                                    .and_then(|c| c.to_hex())
                                    .unwrap_or_else(|| tokens::COLOR_TEXT_PRIMARY.to_string()),
                                border = tokens::COLOR_BORDER_CHROME,
                            ),
                        }
                        span {
                            style: "flex: 1; min-width: 0;",
                            {
                                cp.color
                                    .as_ref()
                                    .and_then(|c| c.to_hex())
                                    .unwrap_or_else(|| fl!("style-dialog-font-colour-automatic"))
                            }
                        }
                    }
                },
                footnote: colour_row
                    .as_ref()
                    .map(|(source, value)| {
                        provenance_line(
                            source,
                            value.as_deref(),
                            posture,
                            open_style,
                            draft,
                            |d| d.style.char_props.color = None,
                        )
                    })
                    .unwrap_or_else(|| rsx! {}),
            }

            // ── Language ──────────────────────────────────────────────────────
            AtField {
                label: fl!("style-dialog-font-language"),
                control: rsx! {
                    div {
                        style: at_control_style(posture.min_touch_px, "width: 100%;"),
                        span {
                            style: "flex: 1; min-width: 0;",
                            {
                                cp.language
                                    .as_ref()
                                    .map(|l| l.as_str().to_string())
                                    .unwrap_or_else(|| fl!("style-dialog-font-language-inherit"))
                            }
                        }
                    }
                },
                footnote: language_row
                    .as_ref()
                    .map(|(source, value)| {
                        provenance_line(
                            source,
                            value.as_deref(),
                            posture,
                            open_style,
                            draft,
                            |d| d.style.char_props.language = None,
                        )
                    })
                    .unwrap_or_else(|| rsx! {}),
            }

            // The count the design puts at the foot of this tab: what a change
            // to an ancestor will *not* reach.
            if local_count > 0 {
                AtDialogNotice {
                    tone: AtNoticeTone::Caution,
                    extra_style: full_width(posture),
                    message: rsx! {
                        { fl!("style-dialog-font-local-count", count = local_count as i64) }
                    },
                }
            }
        }
    }
}

/// Writes a weight/posture preset onto the draft.
///
/// `bold` is derived from the weight rather than set independently: the two
/// disagree in every DOCX round-trip otherwise, since OOXML has no numeric
/// weight. A weight of 400 stores as `None` so the style does not pin every run
/// to Regular — the same rule `draft_to_style` follows.
fn set_posture(d: &mut ParaDialogDraft, weight: u16, italic: bool) {
    d.style.char_props.bold = Some(weight >= 600);
    d.style.char_props.font_weight = if weight == 400 { None } else { Some(weight) };
    d.style.char_props.italic = Some(italic);
}
