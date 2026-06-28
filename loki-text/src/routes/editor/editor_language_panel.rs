// SPDX-License-Identifier: Apache-2.0

//! Inline spelling-language picker panel.
//!
//! Lists the dictionary catalog (bundled + downloadable). The active language is
//! marked; offline-available languages activate immediately; others download on
//! demand. The dictionary's SPDX license is shown next to the Download action so
//! the user sees the terms they accept by downloading (consent is the click —
//! `loki-spell` still enforces the gate and SHA-256 integrity).
//!
//! Docked between canvas and ribbon (no `position: absolute` in Blitz).

use std::sync::{Arc, Mutex};

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_app_shell::spell::{Consent, SpellService};
use loki_i18n::fl;

use crate::editing::cursor::CursorState;
use crate::editing::state::DocumentState;
use crate::routes::editor::editor_spell::{activate_language, download_and_activate};

/// Height of the open language panel in CSS pixels.
pub(super) const LANGUAGE_PANEL_HEIGHT_PX: f32 = 200.0;

/// Renders the language picker when `is_open` is true.
pub(super) fn language_panel(
    doc_state: Arc<Mutex<DocumentState>>,
    cursor_state: Signal<CursorState>,
    service: SpellService,
    mut is_open: Signal<bool>,
    mut status: Signal<Option<String>>,
) -> Element {
    let active = service.language();
    let entries = service.available();

    rsx! {
        div {
            style: format!(
                "height: {h}px; min-height: {h}px; max-height: {h}px; \
                 display: flex; flex-direction: column; flex-shrink: 0; \
                 background: {bg}; border-top: 1px solid {border}; \
                 overflow-y: auto; overflow-x: hidden; padding: {pad}px;",
                h = LANGUAGE_PANEL_HEIGHT_PX,
                bg = tokens::COLOR_SURFACE_1,
                border = tokens::COLOR_BORDER_CHROME,
                pad = tokens::SPACE_2,
            ),

            // Header.
            div {
                style: format!(
                    "display: flex; flex-direction: row; align-items: center; \
                     justify-content: space-between; margin-bottom: {mb}px;",
                    mb = tokens::SPACE_2,
                ),
                span {
                    style: format!(
                        "font-family: {ff}; font-size: {size}px; color: {fg}; font-weight: 600;",
                        ff = tokens::FONT_FAMILY_UI,
                        size = tokens::FONT_SIZE_LABEL,
                        fg = tokens::COLOR_TEXT_ON_CHROME,
                    ),
                    {fl!("editor-spelling-language-title")}
                }
                button {
                    style: format!(
                        "background: transparent; border: none; font-size: {fs}px; \
                         color: {fg}; cursor: pointer; padding: {p}px;",
                        fs = tokens::FONT_SIZE_LABEL,
                        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        p = tokens::SPACE_1,
                    ),
                    onclick: move |_| {
                        is_open.set(false);
                        status.set(None);
                    },
                    "\u{2715}"
                }
            }

            if let Some(msg) = status.read().clone() {
                span { style: muted_style(), "{msg}" }
            }

            // Language rows.
            for entry in entries {
                {
                    let is_active = entry.tag.eq_ignore_ascii_case(&active);
                    let offline = service.is_available_offline(&entry.tag);
                    let doc_state = Arc::clone(&doc_state);
                    let service = service.clone();
                    let tag = entry.tag.clone();
                    let name = format!("{} ({})", entry.native_name, entry.english_name);
                    rsx! {
                        div {
                            style: format!(
                                "display: flex; flex-direction: row; align-items: center; \
                                 justify-content: space-between; gap: 8px; padding: {p}px 0; \
                                 border-bottom: 1px solid {border};",
                                p = tokens::SPACE_1,
                                border = tokens::COLOR_BORDER_CHROME,
                            ),
                            // Name + license.
                            div {
                                style: "display: flex; flex-direction: column;",
                                span {
                                    style: name_style(),
                                    "{name}"
                                }
                                span {
                                    style: muted_style(),
                                    {fl!("editor-spelling-license", license = entry.license_spdx.clone())}
                                }
                            }
                            // Action.
                            if is_active {
                                span {
                                    style: name_style(),
                                    {fl!("editor-spelling-active")}
                                }
                            } else if offline {
                                button {
                                    style: action_style(),
                                    onclick: move |_| {
                                        if !activate_language(&doc_state, cursor_state, &service, &tag) {
                                            status.set(Some(fl!("editor-spelling-load-failed")));
                                        } else {
                                            is_open.set(false);
                                        }
                                    },
                                    {fl!("editor-spelling-use")}
                                }
                            } else {
                                button {
                                    style: action_style(),
                                    onclick: move |_| {
                                        let doc_state = Arc::clone(&doc_state);
                                        let service = service.clone();
                                        let tag = tag.clone();
                                        status.set(Some(fl!("editor-spelling-downloading")));
                                        spawn(async move {
                                            let ok = download_and_activate(
                                                doc_state,
                                                cursor_state,
                                                service,
                                                tag,
                                                Consent::Granted,
                                            )
                                            .await;
                                            status.set(Some(if ok {
                                                fl!("editor-spelling-download-ok")
                                            } else {
                                                fl!("editor-spelling-download-failed")
                                            }));
                                        });
                                    },
                                    {fl!("editor-spelling-download")}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn name_style() -> String {
    format!(
        "font-family: {ff}; font-size: {size}px; color: {fg};",
        ff = tokens::FONT_FAMILY_UI,
        size = tokens::FONT_SIZE_LABEL,
        fg = tokens::COLOR_TEXT_ON_CHROME,
    )
}

fn muted_style() -> String {
    format!(
        "font-family: {ff}; font-size: {size}px; color: {fg};",
        ff = tokens::FONT_FAMILY_UI,
        size = tokens::FONT_SIZE_XS,
        fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
    )
}

fn action_style() -> String {
    format!(
        "padding: {p}px {p2}px; background: {bg}; border: 1px solid {border}; \
         border-radius: 4px; color: {fg}; font-size: {size}px; cursor: pointer; \
         flex-shrink: 0;",
        p = tokens::SPACE_1,
        p2 = tokens::SPACE_3,
        bg = tokens::COLOR_SURFACE_3,
        border = tokens::COLOR_BORDER_CHROME,
        fg = tokens::COLOR_TEXT_ON_CHROME,
        size = tokens::FONT_SIZE_LABEL,
    )
}
