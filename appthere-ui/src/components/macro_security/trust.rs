// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `AtMacroTrustDialog` — the three-choice enable dialog (macro spec §2.3).

use dioxus::prelude::*;

use super::frame::MacroDialogFrame;
use super::{choice_button_style, MacroTrustChoice};
use crate::tokens::colors::{COLOR_MACRO_BADGE, COLOR_TEXT_ON_CHROME_SECONDARY};
use crate::tokens::spacing::{SPACE_1, SPACE_2};
use crate::tokens::typography::FONT_SIZE_BODY;

/// Props for [`AtMacroTrustDialog`]. All display strings are props
/// (i18n-agnostic); the parent maps [`MacroTrustChoice`] to a `MacroService`
/// call.
#[derive(Props, Clone, PartialEq)]
pub struct AtMacroTrustDialogProps {
    /// The word for "Macro" (badge chip).
    pub badge_label: String,
    /// The macro project's name.
    pub project_name: String,
    /// The host document's title.
    pub document_title: String,
    /// Body text explaining the decision (e.g. macros-trust-message).
    pub message: String,
    /// Label for "Keep disabled" (the safe default).
    pub keep_disabled_label: String,
    /// Label for "Enable for this session".
    pub session_label: String,
    /// Label for "Trust this document".
    pub trust_label: String,
    /// Invoked with the user's choice (backdrop click == `KeepDisabled`).
    pub on_choice: EventHandler<MacroTrustChoice>,
}

/// The enable dialog with the three §2.3 choices, rendered in the anti-spoof
/// macro frame.
///
/// Touch targets: every choice button is a full-width row at least 44 logical
/// pixels tall (WCAG 2.5.8) via [`choice_button_style`].
#[component]
pub fn AtMacroTrustDialog(props: AtMacroTrustDialogProps) -> Element {
    // Cloned handles for each button's move-closure.
    let on_keep = props.on_choice;
    let on_session = props.on_choice;
    let on_trust = props.on_choice;
    let on_backdrop = props.on_choice;

    rsx! {
        MacroDialogFrame {
            badge_label: props.badge_label.clone(),
            project_name: props.project_name.clone(),
            document_title: props.document_title.clone(),
            on_backdrop: move |()| on_backdrop.call(MacroTrustChoice::KeepDisabled),

            div {
                style: format!("font-size: {fs}px; color: {fg};", fs = FONT_SIZE_BODY, fg = COLOR_TEXT_ON_CHROME_SECONDARY),
                {props.message.clone()}
            }

            div {
                style: format!("display: flex; flex-direction: column; gap: {gap}px;", gap = SPACE_2),

                // Trust this document (strongest — accented).
                button {
                    style: choice_button_style(true),
                    onclick: move |_| on_trust.call(MacroTrustChoice::TrustAlways),
                    {props.trust_label.clone()}
                }
                // Enable for this session.
                button {
                    style: choice_button_style(false),
                    onclick: move |_| on_session.call(MacroTrustChoice::EnableSession),
                    {props.session_label.clone()}
                }
                // Keep disabled (default / safe).
                button {
                    style: format!(
                        "{base} border-color: {border};",
                        base = choice_button_style(false),
                        border = COLOR_TEXT_ON_CHROME_SECONDARY,
                    ),
                    onclick: move |_| on_keep.call(MacroTrustChoice::KeepDisabled),
                    {props.keep_disabled_label.clone()}
                }
            }

            // A subtle reminder that "Keep disabled" is the safe path.
            div {
                style: format!(
                    "font-size: 11px; color: {fg}; margin-top: {mt}px;",
                    fg = COLOR_MACRO_BADGE, mt = SPACE_1,
                ),
                "aria-hidden": "true",
                "⚡"
            }
        }
    }
}
