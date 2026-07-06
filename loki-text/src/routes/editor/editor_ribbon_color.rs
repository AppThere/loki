// SPDX-License-Identifier: Apache-2.0

//! The Write tab's Font-colour group: an "Automatic" (clear) swatch plus a
//! fixed palette of preset colours. Each swatch is a coloured square rendered
//! inside an [`AtRibbonIconButton`] (which accepts arbitrary children).

use std::sync::{Arc, Mutex};

use appthere_ui::{AtRibbonGroup, AtRibbonIconButton, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::editor_ribbon_format::RibbonEditCtx;
use super::editor_text_color::apply_text_color;
use crate::editing::state::DocumentState;

/// Preset text colours: `(hex, aria-key)`. Readable on a white page.
const PALETTE: &[(&str, &str)] = &[
    ("#C0392B", "ribbon-color-red-aria"),
    ("#E67E22", "ribbon-color-orange-aria"),
    ("#F1C40F", "ribbon-color-yellow-aria"),
    ("#27AE60", "ribbon-color-green-aria"),
    ("#2980B9", "ribbon-color-blue-aria"),
    ("#8E44AD", "ribbon-color-purple-aria"),
];

/// A filled colour-swatch square for a palette button.
fn swatch(hex: &str) -> Element {
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

/// The Font colour group.
pub(super) fn font_color_group(
    doc_state: &Arc<Mutex<DocumentState>>,
    ctx: RibbonEditCtx,
    current: Option<String>,
) -> Element {
    let ds_auto = Arc::clone(doc_state);
    let loro = ctx.loro_doc;
    let cursor = ctx.cursor_state;

    rsx! {
        AtRibbonGroup {
            label:      Some(fl!("ribbon-group-font-color")),
            aria_label: fl!("ribbon-group-font-color"),

            // Automatic: clears the direct colour, reverting to the style colour.
            AtRibbonIconButton {
                aria_label:  fl!("ribbon-color-automatic-aria"),
                is_active:   current.is_none(),
                is_disabled: false,
                on_click: move |_| {
                    if let Some(ldoc) = loro.read().as_ref()
                        && apply_text_color(ldoc, &cursor.read(), None).is_ok()
                    {
                        ctx.finish(&ds_auto, ldoc);
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

            for (hex, aria) in PALETTE.iter().copied() {
                AtRibbonIconButton {
                    key: "{hex}",
                    aria_label:  fl!(aria),
                    is_active:   current.as_deref() == Some(hex),
                    is_disabled: false,
                    on_click: {
                        let ds = Arc::clone(doc_state);
                        move |_| {
                            if let Some(ldoc) = loro.read().as_ref()
                                && apply_text_color(ldoc, &cursor.read(), Some(hex)).is_ok()
                            {
                                ctx.finish(&ds, ldoc);
                            }
                        }
                    },
                    {swatch(hex)}
                }
            }
        }
    }
}
