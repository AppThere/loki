// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `AtNetworkPrompt` — the per-host network-access prompt (ADR-0015 §4.2, §4.6).
//!
//! Distinct from [`super::AtPermissionPrompt`]: `Network` grants are **per
//! origin** and **session-max** (never persisted, ADR-0015 §4.2), so this prompt
//! shows the destination origin verbatim and offers only Deny / Allow-once /
//! Allow-for-this-session — there is no "always for this document". It always
//! carries the **composition warning** (§4.6): because a macro can always read
//! the document (`DocRead` is baseline), granting network is the exfiltration
//! primitive, and the user must see that trade-off before allowing.

use dioxus::prelude::*;

use super::frame::MacroDialogFrame;
use super::{choice_button_style, MacroGrantChoice};
use crate::tokens::colors::{COLOR_STATUS_ERROR_BORDER, COLOR_SURFACE_1, COLOR_TEXT_ON_CHROME};
use crate::tokens::spacing::{RADIUS_SM, SPACE_1, SPACE_2};
use crate::tokens::typography::{
    FONT_FAMILY_UI, FONT_SIZE_BODY, FONT_SIZE_MD, FONT_WEIGHT_SEMIBOLD,
};

/// Props for [`AtNetworkPrompt`]. All display strings are props; the parent maps
/// the emitted [`MacroGrantChoice`] to a `GrantScope` (clamping to session-max).
#[derive(Props, Clone, PartialEq)]
pub struct AtNetworkPromptProps {
    /// The word for "Macro" (badge chip).
    pub badge_label: String,
    /// The macro project's name.
    pub project_name: String,
    /// The host document's title.
    pub document_title: String,
    /// The request headline (e.g. "Allow network access?").
    pub request_title: String,
    /// The destination origin, shown **verbatim** (e.g. `https://api.example.com`).
    pub origin: String,
    /// The composition warning (§4.6) — content can be read and sent to this site.
    pub composition_warning: String,
    /// Label for the default **Deny** button.
    pub deny_label: String,
    /// Label for "Allow once".
    pub allow_once_label: String,
    /// Label for "Allow for this session".
    pub allow_session_label: String,
    /// Invoked with the user's choice (backdrop click == `Deny`).
    pub on_choice: EventHandler<MacroGrantChoice>,
}

/// A per-host network-access prompt asked at first request to an origin.
/// **Deny is the default** (safe) and the backdrop maps to it. Rendered in the
/// anti-spoof [`MacroDialogFrame`] (threat T7).
///
/// Touch targets: every button is at least 44 logical pixels tall (WCAG 2.5.8)
/// via [`choice_button_style`].
//
// TODO(8B.5-homograph): decode a punycode (`xn--`) authority for display and
// flag mixed-script / homograph origins; today the origin is shown verbatim.
#[component]
pub fn AtNetworkPrompt(props: AtNetworkPromptProps) -> Element {
    let on_deny = props.on_choice;
    let on_once = props.on_choice;
    let on_session = props.on_choice;
    let on_backdrop = props.on_choice;

    rsx! {
        MacroDialogFrame {
            badge_label: props.badge_label.clone(),
            project_name: props.project_name.clone(),
            document_title: props.document_title.clone(),
            on_backdrop: move |()| on_backdrop.call(MacroGrantChoice::Deny),

            // Headline.
            div {
                style: format!("font-size: {fs}px; font-weight: {fw}; color: {fg};", fs = FONT_SIZE_MD, fw = FONT_WEIGHT_SEMIBOLD, fg = COLOR_TEXT_ON_CHROME),
                {props.request_title.clone()}
            }
            // Destination origin, shown verbatim in a distinct field so a
            // look-alike host is legible and can't blend into the body copy.
            div {
                style: format!(
                    "font-family: {ui}; font-size: {fs}px; color: {fg}; \
                     background: {bg}; padding: {py}px {px}px; border-radius: {r}px; \
                     word-break: break-all;",
                    ui = FONT_FAMILY_UI, fs = FONT_SIZE_BODY, fg = COLOR_TEXT_ON_CHROME,
                    bg = COLOR_SURFACE_1, py = SPACE_1, px = SPACE_2, r = RADIUS_SM,
                ),
                {props.origin.clone()}
            }
            // Composition warning (§4.6) — error-accented so it reads as a caution.
            div {
                style: format!("font-size: {fs}px; color: {fg};", fs = FONT_SIZE_BODY, fg = COLOR_STATUS_ERROR_BORDER),
                {props.composition_warning.clone()}
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
                // Session is the strongest network grant (never persisted) — accented.
                button {
                    style: choice_button_style(true),
                    onclick: move |_| on_session.call(MacroGrantChoice::AllowSession),
                    {props.allow_session_label.clone()}
                }
            }
        }
    }
}
