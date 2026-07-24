// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `AtPermissionPrompt` — a first-use capability prompt (macro spec §5.4).

use dioxus::prelude::*;

use super::frame::MacroDialogFrame;
use super::{choice_button_style, MacroGrantChoice};
use crate::tokens::colors::{
    COLOR_STATUS_ERROR_BORDER, COLOR_TEXT_ON_CHROME, COLOR_TEXT_ON_CHROME_SECONDARY,
};
use crate::tokens::spacing::{SPACE_1, SPACE_2};
use crate::tokens::typography::{FONT_SIZE_BODY, FONT_SIZE_MD, FONT_WEIGHT_SEMIBOLD};

/// Props for [`AtPermissionPrompt`]. All display strings are props; the parent
/// maps [`MacroGrantChoice`] to a `GrantScope`.
#[derive(Props, Clone, PartialEq)]
pub struct AtPermissionPromptProps {
    /// The word for "Macro" (badge chip).
    pub badge_label: String,
    /// The macro project's name.
    pub project_name: String,
    /// The host document's title.
    pub document_title: String,
    /// Human-readable capability name (e.g. "Change this document").
    pub capability_title: String,
    /// Plain-language consequence line for the capability.
    pub consequence: String,
    /// Label for the default **Deny** button.
    pub deny_label: String,
    /// Label for "Allow once".
    pub allow_once_label: String,
    /// Label for "Allow for this session".
    pub allow_session_label: String,
    /// Label for "Always for this document".
    pub always_label: String,
    /// Invoked with the user's choice (backdrop click == `Deny`).
    pub on_choice: EventHandler<MacroGrantChoice>,
}

/// A capability prompt asked at first use during a run. **Deny is the default**
/// (safe) action and the backdrop maps to it.
///
/// Touch targets: every button is at least 44 logical pixels tall (WCAG 2.5.8)
/// via [`choice_button_style`].
#[component]
pub fn AtPermissionPrompt(props: AtPermissionPromptProps) -> Element {
    let on_deny = props.on_choice;
    let on_once = props.on_choice;
    let on_session = props.on_choice;
    let on_always = props.on_choice;
    let on_backdrop = props.on_choice;

    rsx! {
        MacroDialogFrame {
            badge_label: props.badge_label.clone(),
            project_name: props.project_name.clone(),
            document_title: props.document_title.clone(),
            on_backdrop: move |()| on_backdrop.call(MacroGrantChoice::Deny),

            // What is being requested.
            div {
                style: format!("font-size: {fs}px; font-weight: {fw}; color: {fg};", fs = FONT_SIZE_MD, fw = FONT_WEIGHT_SEMIBOLD, fg = COLOR_TEXT_ON_CHROME),
                {props.capability_title.clone()}
            }
            div {
                style: format!("font-size: {fs}px; color: {fg};", fs = FONT_SIZE_BODY, fg = COLOR_TEXT_ON_CHROME_SECONDARY),
                {props.consequence.clone()}
            }

            div {
                style: format!("display: flex; flex-direction: column; gap: {gap}px; margin-top: {mt}px;", gap = SPACE_2, mt = SPACE_1),

                // Deny — default/safe (error-accented, listed first).
                button {
                    style: format!(
                        "{base} border-color: {border}; color: {border};",
                        base = choice_button_style(false),
                        border = COLOR_STATUS_ERROR_BORDER,
                    ),
                    onclick: move |_| on_deny.call(MacroGrantChoice::Deny),
                    {props.deny_label.clone()}
                }
                button {
                    style: choice_button_style(false),
                    onclick: move |_| on_once.call(MacroGrantChoice::AllowOnce),
                    {props.allow_once_label.clone()}
                }
                button {
                    style: choice_button_style(false),
                    onclick: move |_| on_session.call(MacroGrantChoice::AllowSession),
                    {props.allow_session_label.clone()}
                }
                button {
                    style: choice_button_style(true),
                    onclick: move |_| on_always.call(MacroGrantChoice::AlwaysForDocument),
                    {props.always_label.clone()}
                }
            }
        }
    }
}
