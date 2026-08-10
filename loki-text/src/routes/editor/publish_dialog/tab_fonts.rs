// SPDX-License-Identifier: Apache-2.0

//! The publish dialog's **Fonts** and **Output** tabs.

use std::sync::{Arc, Mutex};

use appthere_ui::{AtDialogNotice, AtField, AtNoticeTone, DialogPosture, at_control_style, tokens};
use dioxus::prelude::*;
use loki_i18n::fl;

use super::body::{grid, span_all};
use super::fonts::{embeddable_count, used_faces};
use crate::editing::state::DocumentState;

/// The **Fonts** tab.
pub(super) fn fonts_tab(doc_state: &Arc<Mutex<DocumentState>>, posture: DialogPosture) -> Element {
    let faces = doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.as_ref().map(|d| used_faces(d)))
        .unwrap_or_default();
    let embeddable = embeddable_count(&faces);
    let device = faces.len() - embeddable;

    rsx! {
        div {
            style: grid(posture),

            div {
                style: format!(
                    "{span} border: 1px solid {border}; border-radius: {r}px; overflow: hidden;",
                    span = span_all(posture),
                    border = tokens::COLOR_BORDER_CHROME,
                    r = tokens::RADIUS_MD,
                ),
                if faces.is_empty() {
                    div {
                        style: format!(
                            "padding: {p}px; font-size: {fs}px; color: {fg};",
                            p = tokens::SPACE_4,
                            fs = tokens::FONT_SIZE_BODY,
                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        ),
                        { fl!("publish-dialog-fonts-none") }
                    }
                }
                for face in faces.iter() {
                    div {
                        key: "{face.name}",
                        style: format!(
                            "display: flex; flex-direction: row; align-items: center; \
                             gap: {gap}px; padding: {py}px {px}px; \
                             border-bottom: 1px solid {border}; font-size: {fs}px; color: {fg};",
                            gap = tokens::SPACE_3,
                            py = tokens::SPACE_2,
                            px = tokens::SPACE_3,
                            border = tokens::COLOR_BORDER_CHROME,
                            fs = tokens::FONT_SIZE_BODY,
                            fg = tokens::COLOR_TEXT_ON_CHROME,
                        ),
                        span { style: "flex: 1; min-width: 0;", {face.name.clone()} }
                        span {
                            style: format!(
                                "flex-shrink: 0; padding: 1px {px}px; border-radius: {r}px; \
                                 border: 1px solid {border}; font-size: {fs}px; color: {fg};",
                                px = tokens::SPACE_1,
                                r = tokens::RADIUS_SM,
                                border = tokens::COLOR_BORDER_CHROME,
                                fs = tokens::FONT_SIZE_XS,
                                fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                            ),
                            if face.bundled {
                                { fl!("publish-dialog-fonts-bundled") }
                            } else {
                                { fl!("publish-dialog-fonts-device") }
                            }
                        }
                        span {
                            style: format!(
                                "flex-shrink: 0; font-size: {fs}px; color: {fg};",
                                fs = tokens::FONT_SIZE_META,
                                fg = if face.bundled {
                                    tokens::COLOR_TEXT_ACCENT
                                } else {
                                    tokens::COLOR_CONTEXTUAL_TAB
                                },
                            ),
                            if face.bundled {
                                { fl!("publish-dialog-fonts-embed") }
                            } else {
                                { fl!("publish-dialog-fonts-cannot") }
                            }
                        }
                    }
                }
            }

            // Note 29: the one place the bundled/device distinction has a
            // licence consequence rather than only a rendering one.
            AtDialogNotice {
                tone: if device > 0 { AtNoticeTone::Caution } else { AtNoticeTone::Info },
                extra_style: span_all(posture),
                message: rsx! {
                    if device > 0 {
                        { fl!("publish-dialog-fonts-substitution", count = device as i64) }
                    } else {
                        { fl!("publish-dialog-fonts-all-bundled") }
                    }
                },
            }

            AtDialogNotice {
                tone: AtNoticeTone::Info,
                extra_style: span_all(posture),
                // TODO(epub-font-embedding): package the bundled faces into the
                // OCF container, with subsetting and IDPF obfuscation.
                message: rsx! { { fl!("publish-dialog-fonts-unsupported") } },
            }
        }
    }
}

/// The **Output** tab.
pub(super) fn output(doc_state: &Arc<Mutex<DocumentState>>, posture: DialogPosture) -> Element {
    let title = doc_state
        .lock()
        .ok()
        .and_then(|s| s.document.as_ref().and_then(|d| d.meta.title.clone()))
        .unwrap_or_else(|| fl!("publish-dialog-untitled"));

    rsx! {
        div {
            style: grid(posture),

            AtField {
                label: fl!("publish-dialog-file-name"),
                extra_style: span_all(posture),
                control: rsx! {
                    div {
                        style: format!(
                            "{base} color: {fg};",
                            base = at_control_style(posture.min_touch_px, "width: 100%;"),
                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        ),
                        span {
                            style: "flex: 1; min-width: 0;",
                            { fl!("publish-dialog-file-name-value", title = title) }
                        }
                    }
                },
                footnote: rsx! {
                    div {
                        style: format!(
                            "display: flex; align-items: center; gap: {gap}px; \
                             font-size: {fs}px; color: {fg};",
                            gap = tokens::SPACE_1,
                            fs = tokens::FONT_SIZE_LABEL,
                            fg = tokens::COLOR_TEXT_ON_CHROME_SECONDARY,
                        ),
                        span { "\u{21B3}" }
                        // The location is chosen in the system save dialog the
                        // export opens, so there is no path field to fill in
                        // here that would not be overridden a moment later.
                        span { { fl!("publish-dialog-location-note") } }
                    }
                },
            }

            AtDialogNotice {
                tone: AtNoticeTone::Info,
                extra_style: span_all(posture),
                message: rsx! { { fl!("publish-dialog-output-unsupported") } },
            }
        }
    }
}
