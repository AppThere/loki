// SPDX-License-Identifier: Apache-2.0

//! The Write tab's colour swatch groups: **Font colour** and **Highlight**.
//!
//! Both are a "clear" swatch (revert to no override) plus a fixed palette of
//! preset colours rendered inside an [`AtRibbonIconButton`] (which accepts
//! arbitrary children). The generic [`swatch_group`] drives both; each group
//! supplies its palette and the mark-apply function.

use std::sync::{Arc, Mutex};

use appthere_ui::{AtRibbonGroup, AtRibbonIconButton, tokens};
use dioxus::prelude::*;
use loki_doc_model::MutationError;
use loki_i18n::fl;
use loro::LoroDoc;

use super::editor_highlight_color::apply_highlight;
use super::editor_ribbon_format::RibbonEditCtx;
use super::editor_text_color::apply_text_color;
use crate::editing::cursor::CursorState;
use crate::editing::state::DocumentState;

/// One preset swatch: the mark `value` written on click, the `fill` colour of
/// the visible square (equal to `value` for font colour; a display colour for a
/// named highlight), and its `aria` key.
#[derive(Clone, Copy)]
pub(super) struct Swatch {
    pub value: &'static str,
    pub fill: &'static str,
    pub aria: &'static str,
}

/// Preset text colours (fill == value, both the hex). Readable on a white page.
const FONT_COLOR_PALETTE: &[Swatch] = &[
    Swatch {
        value: "#C0392B",
        fill: "#C0392B",
        aria: "ribbon-color-red-aria",
    },
    Swatch {
        value: "#E67E22",
        fill: "#E67E22",
        aria: "ribbon-color-orange-aria",
    },
    Swatch {
        value: "#F1C40F",
        fill: "#F1C40F",
        aria: "ribbon-color-yellow-aria",
    },
    Swatch {
        value: "#27AE60",
        fill: "#27AE60",
        aria: "ribbon-color-green-aria",
    },
    Swatch {
        value: "#2980B9",
        fill: "#2980B9",
        aria: "ribbon-color-blue-aria",
    },
    Swatch {
        value: "#8E44AD",
        fill: "#8E44AD",
        aria: "ribbon-color-purple-aria",
    },
];

/// Preset highlight colours: the mark `value` is a `HighlightColor` variant
/// name; the `fill` is the RGB that variant renders as (`resolve::map_highlight_color`).
const HIGHLIGHT_PALETTE: &[Swatch] = &[
    Swatch {
        value: "Yellow",
        fill: "#FFFF00",
        aria: "ribbon-highlight-yellow-aria",
    },
    Swatch {
        value: "Green",
        fill: "#00FF00",
        aria: "ribbon-highlight-green-aria",
    },
    Swatch {
        value: "Cyan",
        fill: "#00FFFF",
        aria: "ribbon-highlight-cyan-aria",
    },
    Swatch {
        value: "Magenta",
        fill: "#FF00FF",
        aria: "ribbon-highlight-magenta-aria",
    },
    Swatch {
        value: "Red",
        fill: "#FF0000",
        aria: "ribbon-highlight-red-aria",
    },
];

/// A colour swatch square filled with `hex`.
fn square(hex: &str) -> Element {
    rsx! {
        div {
            style: format!(
                "width: 18px; height: 18px; border-radius: {r}px; background: {hex}; \
                 border: 1px solid rgba(0,0,0,0.25);",
                r = tokens::RADIUS_SM,
            ),
        }
    }
}

/// A swatch group: a "clear" button (outlined square, applies `None`) plus one
/// filled button per palette entry. `apply` writes the mark for the picked
/// value; `current` is the active mark value (drives the highlighted swatch).
fn swatch_group(
    doc_state: &Arc<Mutex<DocumentState>>,
    ctx: RibbonEditCtx,
    group_aria: String,
    clear_aria: String,
    current: Option<String>,
    palette: &'static [Swatch],
    apply: fn(&LoroDoc, &CursorState, Option<&str>) -> Result<(), MutationError>,
) -> Element {
    let ds_clear = Arc::clone(doc_state);
    let loro = ctx.loro_doc;
    let cursor = ctx.cursor_state;

    rsx! {
        AtRibbonGroup {
            label:      Some(group_aria.clone()),
            aria_label: group_aria,

            AtRibbonIconButton {
                aria_label:  clear_aria,
                is_active:   current.is_none(),
                is_disabled: false,
                on_click: move |_| {
                    if let Some(ldoc) = loro.read().as_ref()
                        && apply(ldoc, &cursor.read(), None).is_ok()
                    {
                        ctx.finish(&ds_clear, ldoc);
                    }
                },
                // An outlined (empty) square = "no colour override".
                div {
                    style: format!(
                        "width: 18px; height: 18px; border-radius: {r}px; background: transparent; \
                         border: 1px solid {b};",
                        r = tokens::RADIUS_SM,
                        b = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                }
            }

            for sw in palette.iter().copied() {
                AtRibbonIconButton {
                    key: "{sw.value}",
                    aria_label:  fl!(sw.aria),
                    is_active:   current.as_deref() == Some(sw.value),
                    is_disabled: false,
                    on_click: {
                        let ds = Arc::clone(doc_state);
                        move |_| {
                            if let Some(ldoc) = loro.read().as_ref()
                                && apply(ldoc, &cursor.read(), Some(sw.value)).is_ok()
                            {
                                ctx.finish(&ds, ldoc);
                            }
                        }
                    },
                    {square(sw.fill)}
                }
            }
        }
    }
}

/// The Font colour group.
pub(super) fn font_color_group(
    doc_state: &Arc<Mutex<DocumentState>>,
    ctx: RibbonEditCtx,
    current: Option<String>,
) -> Element {
    swatch_group(
        doc_state,
        ctx,
        fl!("ribbon-group-font-color"),
        fl!("ribbon-color-automatic-aria"),
        current,
        FONT_COLOR_PALETTE,
        apply_text_color,
    )
}

/// The Highlight colour group.
pub(super) fn highlight_group(
    doc_state: &Arc<Mutex<DocumentState>>,
    ctx: RibbonEditCtx,
    current: Option<String>,
) -> Element {
    swatch_group(
        doc_state,
        ctx,
        fl!("ribbon-group-highlight"),
        fl!("ribbon-highlight-none-aria"),
        current,
        HIGHLIGHT_PALETTE,
        apply_highlight,
    )
}
