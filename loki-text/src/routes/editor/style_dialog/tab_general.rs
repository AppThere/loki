// SPDX-License-Identifier: Apache-2.0

//! The **General** tab: what the style *is* — its name, where it sits in the
//! hierarchy, and what depends on it.
//!
//! # Why the re-resolution count is stated before the commit
//!
//! Re-parenting is the single most consequential edit in this dialog: it can
//! silently change every property the style does not set locally. The count is
//! computed from the draft and shown next to the parent field *while choosing*,
//! not reported afterwards.

use appthere_ui::tokens;
use appthere_ui::{
    AtDialogNotice, AtField, AtNoticeTone, DialogPosture, at_control_style, at_field_label_style,
};
use dioxus::prelude::*;
use loki_doc_model::style::{StyleCatalog, StyleId};
use loki_i18n::fl;

use super::body::ancestry;
use super::fields::{DraftSignal, body_grid_style, full_width, text_field};
use super::rows::{inherited_property_count, local_property_count};

/// Renders the General tab body.
pub(super) fn body(
    catalog: &StyleCatalog,
    id: &StyleId,
    draft: DraftSignal,
    posture: DialogPosture,
) -> Element {
    let Some(current) = draft.read().clone() else {
        return rsx! {};
    };
    let style = current.style.clone();
    let name = style.display_name.clone().unwrap_or_default();
    let parent = style
        .parent
        .as_ref()
        .map(|p| p.as_str().to_string())
        .unwrap_or_default();
    let next = style.next_style_id.clone().unwrap_or_default();

    let inherited = inherited_property_count(&style);
    let local = local_property_count(&style);
    // Only a *changed* parent re-resolves anything; an untouched field must not
    // warn about a consequence the user has not asked for.
    let reparenting = style.parent != current.original.parent;
    let dependents = dependent_names(catalog, id);
    let builtin = style.is_builtin();
    let chain = ancestry(catalog, id);

    // Built before the `rsx!` block: a prop value is an expression position, and
    // an `if` there is not one.
    let reparent_note: Element = if reparenting {
        rsx! {
            div {
                style: format!(
                    "display: flex; align-items: center; gap: {gap}px; \
                     font-size: {fs}px; color: {fg};",
                    gap = tokens::SPACE_1,
                    fs = tokens::FONT_SIZE_LABEL,
                    fg = tokens::COLOR_CONTEXTUAL_TAB,
                ),
                span { "\u{203B}" }
                span { { fl!("style-dialog-general-reresolve", count = inherited as i64) } }
            }
        }
    } else {
        rsx! {}
    };

    rsx! {
        div {
            style: body_grid_style(posture),

            // A built-in style's name is part of the format contract — renaming
            // it breaks the mapping an import/export round-trip relies on.
            { text_field(
                fl!("style-dialog-general-name"),
                name,
                draft,
                posture,
                move |d, v| {
                    d.style.display_name = if v.trim().is_empty() { None } else { Some(v) };
                },
                String::new(),
            ) }

            AtField {
                label: fl!("style-dialog-general-inherits-from"),
                control: rsx! {
                    div {
                        style: at_control_style(posture.min_touch_px, "width: 100%;"),
                        input {
                            r#type: "text",
                            value: "{parent}",
                            style: format!(
                                "flex: 1; min-width: 0; background: transparent; border: none; \
                                 font-size: {fs}px; color: {fg};",
                                fs = tokens::FONT_SIZE_MD,
                                fg = tokens::COLOR_TEXT_ON_CHROME,
                            ),
                            oninput: move |evt| {
                                let mut draft = draft;
                                let mut next = draft.read().clone();
                                if let Some(d) = next.as_mut() {
                                    let v = evt.value();
                                    d.style.parent = if v.trim().is_empty() {
                                        None
                                    } else {
                                        Some(StyleId::new(v.trim()))
                                    };
                                }
                                draft.set(next);
                            },
                        }
                    }
                },
                footnote: reparent_note,
            }

            { text_field(
                fl!("style-dialog-general-next"),
                next,
                draft,
                posture,
                move |d, v| {
                    // Trimmed like the parent field above: an id pasted with
                    // surrounding whitespace otherwise never resolves.
                    let v = v.trim();
                    d.style.next_style_id =
                        if v.is_empty() { None } else { Some(v.to_string()) };
                },
                String::new(),
            ) }

            // The format-contract note sits where the rename happens.
            if builtin {
                AtDialogNotice {
                    tone: AtNoticeTone::Info,
                    message: rsx! { { fl!("style-dialog-general-builtin") } },
                }
            }

            // ── Inheritance chain ─────────────────────────────────────────────
            div {
                style: format!(
                    "display: flex; flex-direction: column; gap: {gap}px; {span}",
                    gap = tokens::SPACE_2,
                    span = full_width(posture),
                ),
                div { style: at_field_label_style(), { fl!("style-dialog-general-chain") } }
                div {
                    style: format!(
                        "display: flex; flex-direction: row; align-items: center; \
                         flex-wrap: wrap; gap: {gap}px; padding: {py}px {px}px; \
                         background: {bg}; border: 1px solid {border}; \
                         border-radius: {r}px; font-size: {fs}px; color: {fg};",
                        gap = tokens::SPACE_2,
                        py = tokens::SPACE_3,
                        px = tokens::SPACE_3,
                        bg = tokens::COLOR_SURFACE_2,
                        border = tokens::COLOR_BORDER_CHROME,
                        r = tokens::RADIUS_MD,
                        fs = tokens::FONT_SIZE_BODY,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    // One keyed element per chain link — the separator lives
                    // inside it, because a key is only honoured on the first
                    // node of a block.
                    for (i, (sid, display)) in chain.iter().enumerate() {
                        span {
                            key: "node-{sid.as_str()}",
                            style: format!(
                                "display: flex; flex-direction: row; align-items: center; gap: {gap}px;",
                                gap = tokens::SPACE_2,
                            ),
                            if i > 0 {
                                span { "\u{2192}" }
                            }
                            span {
                                style: if sid == id {
                                    format!(
                                        "padding-left: {p}px; border-left: 3px solid {accent}; color: {fg};",
                                        p = tokens::SPACE_2,
                                        accent = tokens::COLOR_TAB_ACTIVE_INDICATOR,
                                        fg = tokens::COLOR_TEXT_ON_CHROME,
                                    )
                                } else {
                                    String::new()
                                },
                                {display.clone()}
                            }
                        }
                    }
                    span {
                        style: format!("margin-left: auto; font-size: {}px;", tokens::FONT_SIZE_META),
                        {
                            fl!(
                                "style-dialog-general-counts",
                                local = local as i64,
                                inherited = inherited as i64
                            )
                        }
                    }
                }
            }

            AtDialogNotice {
                tone: if dependents.is_empty() { AtNoticeTone::Info } else { AtNoticeTone::Caution },
                extra_style: full_width(posture),
                message: rsx! {
                    {
                        fl!(
                            "style-dialog-general-dependents",
                            count = dependents.len() as i64,
                            names = dependents.join(", ")
                        )
                    }
                },
            }
        }
    }
}

/// Display names of the styles that name `id` as their parent.
///
/// Direct children only, not the transitive closure: these are the styles a
/// change here reaches *first*, and the number a user can reason about. A
/// transitive count on a deep catalog is a large number with no actionable
/// meaning.
fn dependent_names(catalog: &StyleCatalog, id: &StyleId) -> Vec<String> {
    let mut names: Vec<String> = catalog
        .paragraph_styles
        .values()
        .filter(|s| s.parent.as_ref() == Some(id))
        .map(|s| {
            s.display_name
                .clone()
                .unwrap_or_else(|| s.id.as_str().to_string())
        })
        .collect();
    names.sort();
    names
}
