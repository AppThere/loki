// SPDX-License-Identifier: Apache-2.0

//! Paragraph style catalog editor panel for the document editor.
//!
//! `style_editor_panel` renders a two-column panel (style list + form) above
//! the ribbon when `editing_style_draft` is `Some`.  Changes are written to a
//! [`StyleDraft`] signal and committed to the catalog via the Apply button.

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_doc_model::content::attr::ExtensionBag;
use loki_doc_model::loki_primitives::units::Points;
use loki_doc_model::style::ParagraphStyle;
use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::char_props::UnderlineStyle;
use loki_doc_model::style::props::para_props::{ParagraphAlignment, Spacing};
use loki_doc_model::style::props::{CharProps, ParaProps};
use loki_i18n::fl;

use super::editor_state::StyleDraft;
use super::editor_style_catalog::{
    catalog_style_list, get_catalog_style, new_custom_style_id, upsert_catalog_style,
};
use crate::editing::state::{DocumentState, apply_mutation_and_relayout};

/// Height of the open style editor panel in CSS pixels.
pub(super) const STYLE_EDITOR_HEIGHT_PX: f32 = 240.0;

/// Converts a catalog `ParagraphStyle` to an editable `StyleDraft`.
pub(super) fn style_to_draft(style: &ParagraphStyle) -> StyleDraft {
    StyleDraft {
        id: style.id.as_str().to_string(),
        name: style
            .display_name
            .clone()
            .unwrap_or_else(|| style.id.as_str().to_string()),
        parent: style
            .parent
            .as_ref()
            .map(|p| p.as_str().to_string())
            .unwrap_or_default(),
        next: style.next_style_id.clone().unwrap_or_default(),
        alignment: match style.para_props.alignment {
            Some(ParagraphAlignment::Center) => "Center",
            Some(ParagraphAlignment::Right) => "Right",
            Some(ParagraphAlignment::Justify) => "Justify",
            _ => "Left",
        }
        .to_string(),
        font_size_str: style
            .char_props
            .font_size
            .map(|s| format!("{:.0}", s.value()))
            .unwrap_or_default(),
        bold: style.char_props.bold.unwrap_or(false),
        italic: style.char_props.italic.unwrap_or(false),
        underline: style.char_props.underline.is_some(),
        space_before_str: match style.para_props.space_before {
            Some(Spacing::Exact(pt)) => format!("{:.0}", pt.value()),
            _ => String::new(),
        },
        space_after_str: match style.para_props.space_after {
            Some(Spacing::Exact(pt)) => format!("{:.0}", pt.value()),
            _ => String::new(),
        },
        indent_first_str: style
            .para_props
            .indent_first_line
            .map(|pt| format!("{:.0}", pt.value()))
            .unwrap_or_default(),
        is_custom: style.is_custom,
    }
}

/// Converts a `StyleDraft` back to a `ParagraphStyle` for catalog storage.
pub(super) fn draft_to_style(draft: &StyleDraft) -> ParagraphStyle {
    let alignment = match draft.alignment.as_str() {
        "Center" => Some(ParagraphAlignment::Center),
        "Right" => Some(ParagraphAlignment::Right),
        "Justify" => Some(ParagraphAlignment::Justify),
        "Left" => Some(ParagraphAlignment::Left),
        _ => None,
    };
    ParagraphStyle {
        id: StyleId::new(&draft.id),
        display_name: if draft.name.is_empty() {
            None
        } else {
            Some(draft.name.clone())
        },
        parent: if draft.parent.is_empty() {
            None
        } else {
            Some(StyleId::new(&draft.parent))
        },
        linked_char_style: None,
        next_style_id: if draft.next.is_empty() {
            None
        } else {
            Some(draft.next.clone())
        },
        para_props: ParaProps {
            alignment,
            space_before: draft
                .space_before_str
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|&s| s >= 0.0)
                .map(|s| Spacing::Exact(Points::new(s))),
            space_after: draft
                .space_after_str
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|&s| s >= 0.0)
                .map(|s| Spacing::Exact(Points::new(s))),
            indent_first_line: draft
                .indent_first_str
                .trim()
                .parse::<f64>()
                .ok()
                .map(Points::new),
            ..Default::default()
        },
        char_props: CharProps {
            bold: Some(draft.bold),
            italic: Some(draft.italic),
            underline: if draft.underline {
                Some(UnderlineStyle::Single)
            } else {
                None
            },
            font_size: draft
                .font_size_str
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|&s| s > 0.0)
                .map(Points::new),
            ..Default::default()
        },
        is_default: false,
        is_custom: draft.is_custom,
        extensions: ExtensionBag::default(),
    }
}

/// Renders the inline style catalog editor panel.
///
/// Plain function — no hooks.  Left column shows all catalog styles; right
/// column shows an edit form for the currently selected draft.  Apply commits
/// the draft to the catalog and triggers a full relayout.
pub(super) fn style_editor_panel(
    doc_state: Arc<Mutex<DocumentState>>,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    mut editing_style_draft: Signal<Option<StyleDraft>>,
) -> Element {
    let draft = match editing_style_draft.read().clone() {
        Some(d) => d,
        None => return rsx! {},
    };

    let styles = catalog_style_list(&doc_state);
    let ds_list = Arc::clone(&doc_state);
    let ds_new = Arc::clone(&doc_state);
    let ds_apply = Arc::clone(&doc_state);
    let draft_alignment = draft.alignment.clone();
    let biu = [
        ("B", draft.bold),
        ("I", draft.italic),
        ("U", draft.underline),
    ];

    rsx! {
        div {
            style: format!(
                "height: {h}px; min-height: {h}px; max-height: {h}px; \
                 display: flex; flex-direction: column; flex-shrink: 0; \
                 background: {bg}; border-top: 1px solid {border};",
                h = STYLE_EDITOR_HEIGHT_PX,
                bg = tokens::COLOR_SURFACE_1,
                border = tokens::COLOR_BORDER_CHROME,
            ),

            // ── Header ────────────────────────────────────────────────────────
            div {
                style: format!(
                    "display: flex; flex-direction: row; align-items: center; \
                     justify-content: space-between; padding: 0 {p}px; \
                     flex-shrink: 0; height: 28px;",
                    p = tokens::SPACE_4,
                ),
                span {
                    style: format!(
                        "font-family: {ff}; font-size: {fs}px; font-weight: {fw}; color: {fg};",
                        ff = tokens::FONT_FAMILY_UI,
                        fs = tokens::FONT_SIZE_LABEL,
                        fw = tokens::FONT_WEIGHT_MEDIUM,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    { fl!("ribbon-style-editor-heading") }
                }
                button {
                    style: format!(
                        "background: transparent; border: none; font-size: {fs}px; \
                         color: {fg}; cursor: pointer; padding: {p}px;",
                        fs = tokens::FONT_SIZE_LABEL,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        p = tokens::SPACE_1,
                    ),
                    aria_label: fl!("editor-style-editor-close-aria"),
                    onclick: move |_| editing_style_draft.set(None),
                    "\u{2715}"
                }
            }

            // ── Two-column body ────────────────────────────────────────────────
            div {
                style: "display: flex; flex-direction: row; flex: 1; overflow: hidden;",

                // ── Left: catalog style list ───────────────────────────────────
                div {
                    style: format!(
                        "width: 160px; min-width: 160px; overflow-y: auto; \
                         border-right: 1px solid {border}; display: flex; \
                         flex-direction: column; gap: 2px; padding: {p}px;",
                        border = tokens::COLOR_BORDER_CHROME,
                        p = tokens::SPACE_2,
                    ),

                    {styles.into_iter().map(|(id, display)| {
                        let is_active = id == draft.id;
                        let ds_c = Arc::clone(&ds_list);
                        let id_cap = id.clone();
                        rsx! {
                            button {
                                key: "{id}",
                                style: format!(
                                    "text-align: left; padding: {p}px {p2}px; \
                                     border-radius: 3px; border: 1px solid {border}; \
                                     cursor: pointer; font-family: {ff}; \
                                     font-size: {fs}px; background: {bg}; color: {fg};",
                                    p = tokens::SPACE_1, p2 = tokens::SPACE_2,
                                    border = if is_active { tokens::COLOR_TAB_ACTIVE_INDICATOR } else { tokens::COLOR_BORDER_CHROME },
                                    ff = tokens::FONT_FAMILY_UI,
                                    fs = tokens::FONT_SIZE_LABEL,
                                    bg = if is_active { tokens::COLOR_SURFACE_3 } else { tokens::COLOR_SURFACE_2 },
                                    fg = tokens::COLOR_TEXT_ON_CHROME,
                                ),
                                onclick: move |_| {
                                    if let Some(s) = get_catalog_style(&ds_c, &id_cap) {
                                        editing_style_draft.set(Some(style_to_draft(&s)));
                                    }
                                },
                                "{display}"
                            }
                        }
                    })}

                    button {
                        style: format!(
                            "padding: {p}px {p2}px; border-radius: 3px; margin-top: {mt}px; \
                             border: 1px solid {border}; cursor: pointer; \
                             font-family: {ff}; font-size: {fs}px; \
                             background: {bg}; color: {fg};",
                            p = tokens::SPACE_1, p2 = tokens::SPACE_2,
                            mt = tokens::SPACE_2,
                            border = tokens::COLOR_BORDER_DEFAULT,
                            ff = tokens::FONT_FAMILY_UI,
                            fs = tokens::FONT_SIZE_LABEL,
                            bg = tokens::COLOR_SURFACE_2,
                            fg = tokens::COLOR_TEXT_ON_CHROME,
                        ),
                        aria_label: fl!("ribbon-style-new-aria"),
                        onclick: move |_| {
                            let new_id = new_custom_style_id(&ds_new);
                            editing_style_draft.set(Some(StyleDraft {
                                id: new_id.clone(),
                                name: new_id,
                                is_custom: true,
                                alignment: "Left".to_string(),
                                ..StyleDraft::default()
                            }));
                        },
                        "+ New"
                    }
                }

                // ── Right: edit form ───────────────────────────────────────────
                div {
                    style: format!(
                        "flex: 1; display: flex; flex-direction: column; \
                         gap: {g}px; padding: {p}px; overflow-y: auto;",
                        g = tokens::SPACE_2,
                        p = tokens::SPACE_3,
                    ),

                    // Name
                    div {
                        style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
                        span {
                            style: format!(
                                "font-family: {ff}; font-size: {fs}px; color: {fg}; min-width: 64px;",
                                ff = tokens::FONT_FAMILY_UI,
                                fs = tokens::FONT_SIZE_LABEL,
                                fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                            ),
                            "Name"
                        }
                        input {
                            r#type: "text",
                            value: "{draft.name}",
                            oninput: move |evt| {
                                let v = editing_style_draft.read().clone();
                                if let Some(mut d) = v { d.name = evt.value(); editing_style_draft.set(Some(d)); }
                            },
                            style: format!(
                                "flex: 1; height: 24px; padding: 0 {p}px; \
                                 background: {bg}; border: 1px solid {border}; \
                                 border-radius: {r}px; font-family: {ff}; \
                                 font-size: {fs}px; color: {fg}; box-sizing: border-box;",
                                p = tokens::SPACE_2,
                                bg = tokens::COLOR_SURFACE_2,
                                border = tokens::COLOR_BORDER_DEFAULT,
                                r = tokens::RADIUS_SM,
                                ff = tokens::FONT_FAMILY_UI,
                                fs = tokens::FONT_SIZE_BODY,
                                fg = tokens::COLOR_TEXT_ON_CHROME,
                            ),
                        }
                    }

                    // Based on + Next style
                    div {
                        style: "display: flex; flex-direction: row; align-items: center; gap: 16px;",
                        div {
                            style: "display: flex; flex-direction: row; align-items: center; gap: 8px; flex: 1;",
                            span {
                                style: format!("font-family: {ff}; font-size: {fs}px; color: {fg}; min-width: 56px;", ff = tokens::FONT_FAMILY_UI, fs = tokens::FONT_SIZE_LABEL, fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY),
                                { fl!("editor-style-based-on-label") }
                            }
                            input {
                                r#type: "text",
                                value: "{draft.parent}",
                                oninput: move |evt| {
                                    let v = editing_style_draft.read().clone();
                                    if let Some(mut d) = v { d.parent = evt.value(); editing_style_draft.set(Some(d)); }
                                },
                                style: format!("flex: 1; height: 24px; padding: 0 {p}px; background: {bg}; border: 1px solid {border}; border-radius: {r}px; font-family: {ff}; font-size: {fs}px; color: {fg}; box-sizing: border-box;", p = tokens::SPACE_2, bg = tokens::COLOR_SURFACE_2, border = tokens::COLOR_BORDER_DEFAULT, r = tokens::RADIUS_SM, ff = tokens::FONT_FAMILY_UI, fs = tokens::FONT_SIZE_BODY, fg = tokens::COLOR_TEXT_ON_CHROME),
                            }
                        }
                        div {
                            style: "display: flex; flex-direction: row; align-items: center; gap: 8px; flex: 1;",
                            span {
                                style: format!("font-family: {ff}; font-size: {fs}px; color: {fg}; min-width: 56px;", ff = tokens::FONT_FAMILY_UI, fs = tokens::FONT_SIZE_LABEL, fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY),
                                { fl!("editor-style-next-style-label") }
                            }
                            input {
                                r#type: "text",
                                value: "{draft.next}",
                                oninput: move |evt| {
                                    let v = editing_style_draft.read().clone();
                                    if let Some(mut d) = v { d.next = evt.value(); editing_style_draft.set(Some(d)); }
                                },
                                style: format!("flex: 1; height: 24px; padding: 0 {p}px; background: {bg}; border: 1px solid {border}; border-radius: {r}px; font-family: {ff}; font-size: {fs}px; color: {fg}; box-sizing: border-box;", p = tokens::SPACE_2, bg = tokens::COLOR_SURFACE_2, border = tokens::COLOR_BORDER_DEFAULT, r = tokens::RADIUS_SM, ff = tokens::FONT_FAMILY_UI, fs = tokens::FONT_SIZE_BODY, fg = tokens::COLOR_TEXT_ON_CHROME),
                            }
                        }
                    }

                    // Alignment + Font size + B/I/U
                    div {
                        style: "display: flex; flex-direction: row; align-items: center; gap: 16px; flex-wrap: wrap;",
                        div {
                            style: "display: flex; flex-direction: row; align-items: center; gap: 4px;",
                            span {
                                style: format!("font-family: {ff}; font-size: {fs}px; color: {fg}; margin-right: 4px;", ff = tokens::FONT_FAMILY_UI, fs = tokens::FONT_SIZE_LABEL, fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY),
                                { fl!("editor-style-align-label") }
                            }
                            {["Left", "Center", "Right", "Justify"].iter().map(|&aval| {
                                let is_a = draft_alignment.as_str() == aval;
                                rsx! {
                                    button {
                                        key: "{aval}",
                                        style: format!(
                                            "padding: 2px 6px; border-radius: 3px; border: 1px solid {border}; \
                                             cursor: pointer; font-family: {ff}; font-size: {fs}px; \
                                             background: {bg}; color: {fg};",
                                            border = if is_a { tokens::COLOR_TAB_ACTIVE_INDICATOR } else { tokens::COLOR_BORDER_CHROME },
                                            ff = tokens::FONT_FAMILY_UI,
                                            fs = tokens::FONT_SIZE_LABEL,
                                            bg = if is_a { tokens::COLOR_SURFACE_3 } else { tokens::COLOR_SURFACE_2 },
                                            fg = tokens::COLOR_TEXT_ON_CHROME,
                                        ),
                                        onclick: move |_| {
                                            let v = editing_style_draft.read().clone();
                                            if let Some(mut d) = v { d.alignment = aval.to_string(); editing_style_draft.set(Some(d)); }
                                        },
                                        "{aval}"
                                    }
                                }
                            })}
                        }
                        div {
                            style: "display: flex; flex-direction: row; align-items: center; gap: 4px;",
                            span {
                                style: format!("font-family: {ff}; font-size: {fs}px; color: {fg};", ff = tokens::FONT_FAMILY_UI, fs = tokens::FONT_SIZE_LABEL, fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY),
                                "Size (pt)"
                            }
                            input {
                                r#type: "text",
                                value: "{draft.font_size_str}",
                                oninput: move |evt| {
                                    let v = editing_style_draft.read().clone();
                                    if let Some(mut d) = v { d.font_size_str = evt.value(); editing_style_draft.set(Some(d)); }
                                },
                                style: format!("width: 40px; height: 24px; padding: 0 {p}px; background: {bg}; border: 1px solid {border}; border-radius: {r}px; font-family: {ff}; font-size: {fs}px; color: {fg}; box-sizing: border-box;", p = tokens::SPACE_2, bg = tokens::COLOR_SURFACE_2, border = tokens::COLOR_BORDER_DEFAULT, r = tokens::RADIUS_SM, ff = tokens::FONT_FAMILY_UI, fs = tokens::FONT_SIZE_BODY, fg = tokens::COLOR_TEXT_ON_CHROME),
                            }
                        }
                        div {
                            style: "display: flex; flex-direction: row; align-items: center; gap: 4px;",
                            {biu.into_iter().map(|(lbl, active)| {
                                rsx! {
                                    button {
                                        key: "{lbl}",
                                        style: format!(
                                            "padding: 2px 8px; border-radius: 3px; border: 1px solid {border}; \
                                             cursor: pointer; font-family: {ff}; font-size: {fs}px; \
                                             font-weight: {fw}; font-style: {fi}; text-decoration: {td}; \
                                             background: {bg}; color: {fg};",
                                            border = if active { tokens::COLOR_TAB_ACTIVE_INDICATOR } else { tokens::COLOR_BORDER_CHROME },
                                            ff = tokens::FONT_FAMILY_UI,
                                            fs = tokens::FONT_SIZE_LABEL,
                                            fw = if lbl == "B" { "700" } else { "400" },
                                            fi = if lbl == "I" { "italic" } else { "normal" },
                                            td = if lbl == "U" { "underline" } else { "none" },
                                            bg = if active { tokens::COLOR_SURFACE_3 } else { tokens::COLOR_SURFACE_2 },
                                            fg = tokens::COLOR_TEXT_ON_CHROME,
                                        ),
                                        onclick: move |_| {
                                            let v = editing_style_draft.read().clone();
                                            if let Some(mut d) = v {
                                                match lbl {
                                                    "B" => d.bold = !d.bold,
                                                    "I" => d.italic = !d.italic,
                                                    "U" => d.underline = !d.underline,
                                                    _ => {}
                                                }
                                                editing_style_draft.set(Some(d));
                                            }
                                        },
                                        "{lbl}"
                                    }
                                }
                            })}
                        }
                    }

                    // Apply
                    div {
                        style: "display: flex; flex-direction: row; gap: 8px; margin-top: auto;",
                        button {
                            style: format!(
                                "padding: {p}px {p2}px; border-radius: {r}px; \
                                 border: 1px solid {border}; cursor: pointer; \
                                 font-family: {ff}; font-size: {fs}px; \
                                 background: {bg}; color: {fg};",
                                p = tokens::SPACE_1,
                                p2 = tokens::SPACE_3,
                                r = tokens::RADIUS_SM,
                                border = tokens::COLOR_TAB_ACTIVE_INDICATOR,
                                ff = tokens::FONT_FAMILY_UI,
                                fs = tokens::FONT_SIZE_BODY,
                                bg = tokens::COLOR_SURFACE_3,
                                fg = tokens::COLOR_TEXT_ON_CHROME,
                            ),
                            onclick: move |_| {
                                let v = editing_style_draft.read().clone();
                                if let Some(draft_val) = v {
                                    let style = draft_to_style(&draft_val);
                                    upsert_catalog_style(&ds_apply, style);
                                    let ldoc_guard = loro_doc.read();
                                    if let Some(ldoc) = ldoc_guard.as_ref() {
                                        apply_mutation_and_relayout(&ds_apply, ldoc);
                                    }
                                }
                            },
                            { fl!("ribbon-style-apply-changes") }
                        }
                    }
                }
            }
        }
    }
}
