// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Live rendering of a running macro's capability prompts and dialogs (macro
//! spec §5.4, §5.5).
//!
//! The worker thread posts a [`UiRequest`] via the bridge; the runner mounts
//! this component to render it in the anti-spoof frame and emits the user's
//! answer as a [`UiReply`]. A capability prompt uses `AtPermissionPrompt`; a
//! `MsgBox`/`InputBox` uses the badged `MacroDialogFrame` — both frames app
//! chrome never uses, so a macro dialog can't impersonate a real one.

use appthere_ui::{
    AtNetworkPrompt, AtPermissionPrompt, MacroDialogFrame, MacroGrantChoice, tokens,
};
use dioxus::prelude::*;
use loki_i18n::fl;
use loki_macro_host::{Capability, DialogKind, DialogOutcome, GrantScope};

use super::editor_macro_bridge::{UiReply, UiRequest};

/// A render-friendly (Clone) view of a pending [`UiRequest`].
#[derive(Clone, PartialEq)]
pub(super) enum PromptKind {
    /// A first-use capability prompt.
    Capability(Capability),
    /// A first-request-per-origin network prompt carrying the destination origin.
    Network(String),
    /// A `MsgBox` — message + optional title.
    Message {
        prompt: String,
        title: Option<String>,
    },
    /// An `InputBox` — prompt + optional title + default text.
    Input {
        prompt: String,
        title: Option<String>,
        default: String,
    },
    /// A request handled inline by the runner's drain loop (a file pick), never
    /// rendered as a prompt. Present only to keep [`PromptKind::from_request`]
    /// total; [`MacroPromptView`] renders nothing for it.
    Internal,
}

impl PromptKind {
    /// Derives a render view from a bridge request.
    pub(super) fn from_request(req: &UiRequest) -> Self {
        match req {
            UiRequest::Capability(cap) => PromptKind::Capability(*cap),
            UiRequest::Network(origin) => PromptKind::Network(origin.clone()),
            // File requests are consumed by the drain loop, never rendered.
            UiRequest::PickReadFile(_)
            | UiRequest::PickWriteTarget(_)
            | UiRequest::WriteFile { .. } => PromptKind::Internal,
            UiRequest::Dialog(d) => match d.kind {
                DialogKind::Message => PromptKind::Message {
                    prompt: d.prompt.clone(),
                    title: d.title.clone(),
                },
                DialogKind::Input => PromptKind::Input {
                    prompt: d.prompt.clone(),
                    title: d.title.clone(),
                    default: d.default.clone().unwrap_or_default(),
                },
            },
        }
    }
}

/// Renders the pending prompt and emits the user's [`UiReply`].
#[component]
pub(super) fn MacroPromptView(
    kind: PromptKind,
    project: String,
    doc_title: String,
    on_answer: EventHandler<UiReply>,
) -> Element {
    match kind {
        PromptKind::Capability(cap) => rsx! {
            AtPermissionPrompt {
                badge_label: fl!("macros-badge"),
                project_name: project,
                document_title: doc_title,
                capability_title: fl!(&format!("macros-cap-{}-title", cap.id())),
                consequence: fl!(&format!("macros-cap-{}-consequence", cap.id())),
                deny_label: fl!("macros-perm-deny"),
                allow_once_label: fl!("macros-perm-allow-once"),
                allow_session_label: fl!("macros-perm-allow-session"),
                always_label: fl!("macros-perm-allow-always"),
                on_choice: move |choice: MacroGrantChoice| {
                    on_answer.call(UiReply::Grant(scope_of(choice)));
                },
            }
        },
        PromptKind::Network(origin) => rsx! {
            AtNetworkPrompt {
                badge_label: fl!("macros-badge"),
                project_name: project,
                document_title: doc_title,
                request_title: fl!("macros-net-title"),
                origin,
                composition_warning: fl!("macros-net-warning"),
                deny_label: fl!("macros-net-deny"),
                allow_once_label: fl!("macros-net-allow-once"),
                allow_session_label: fl!("macros-net-allow-session"),
                on_choice: move |choice: MacroGrantChoice| {
                    on_answer.call(UiReply::Grant(scope_of(choice)));
                },
            }
        },
        PromptKind::Message { prompt, title } => rsx! {
            MacroDialogFrame {
                badge_label: fl!("macros-badge"),
                project_name: project,
                document_title: title.unwrap_or(doc_title),
                on_backdrop: move |()| on_answer.call(UiReply::Dialog(DialogOutcome::Cancelled)),
                div { style: message_style(), "{prompt}" }
                div { style: button_row(),
                    button {
                        style: dialog_button(false),
                        onclick: move |_| on_answer.call(UiReply::Dialog(DialogOutcome::Button(2))),
                        {fl!("macros-dialog-cancel")}
                    }
                    button {
                        style: dialog_button(true),
                        onclick: move |_| on_answer.call(UiReply::Dialog(DialogOutcome::Button(1))),
                        {fl!("macros-dialog-ok")}
                    }
                }
            }
        },
        PromptKind::Input {
            prompt,
            title,
            default,
        } => rsx! {
            InputDialog {
                prompt,
                title: title.unwrap_or(doc_title.clone()),
                default,
                project,
                on_answer,
            }
        },
        // A file pick is serviced by the drain loop, not rendered.
        PromptKind::Internal => rsx! {},
    }
}

/// An `InputBox` dialog with a text field (its own signal for the entry).
#[component]
fn InputDialog(
    prompt: String,
    title: String,
    default: String,
    project: String,
    on_answer: EventHandler<UiReply>,
) -> Element {
    let mut value = use_signal(|| default.clone());
    rsx! {
        MacroDialogFrame {
            badge_label: fl!("macros-badge"),
            project_name: project,
            document_title: title,
            on_backdrop: move |()| on_answer.call(UiReply::Dialog(DialogOutcome::Cancelled)),
            div { style: message_style(), "{prompt}" }
            input {
                style: input_style(),
                value: "{value}",
                oninput: move |e| value.set(e.value()),
            }
            div { style: button_row(),
                button {
                    style: dialog_button(false),
                    onclick: move |_| on_answer.call(UiReply::Dialog(DialogOutcome::Cancelled)),
                    {fl!("macros-dialog-cancel")}
                }
                button {
                    style: dialog_button(true),
                    onclick: move |_| on_answer.call(UiReply::Dialog(DialogOutcome::Text(value()))),
                    {fl!("macros-dialog-ok")}
                }
            }
        }
    }
}

fn scope_of(choice: MacroGrantChoice) -> GrantScope {
    match choice {
        MacroGrantChoice::Deny => GrantScope::Deny,
        MacroGrantChoice::AllowOnce => GrantScope::AllowOnce,
        MacroGrantChoice::AllowSession => GrantScope::AllowSession,
        MacroGrantChoice::AlwaysForDocument => GrantScope::AlwaysForDocument,
    }
}

fn message_style() -> String {
    format!(
        "font-size: {}px; color: {};",
        tokens::FONT_SIZE_BODY,
        tokens::COLOR_TEXT_ON_CHROME
    )
}

fn input_style() -> String {
    format!(
        "min-height: {th}px; box-sizing: border-box; width: 100%; padding: {p}px; \
         background: {bg}; border: 1px solid {border}; border-radius: {r}px; \
         color: {fg}; font-size: {s}px;",
        th = tokens::TOUCH_MIN,
        p = tokens::SPACE_2,
        bg = tokens::COLOR_SURFACE_1,
        border = tokens::COLOR_BORDER_CHROME,
        r = tokens::RADIUS_SM,
        fg = tokens::COLOR_TEXT_ON_CHROME,
        s = tokens::FONT_SIZE_BODY,
    )
}

fn button_row() -> String {
    format!(
        "display: flex; flex-direction: row; justify-content: flex-end; gap: {}px;",
        tokens::SPACE_2
    )
}

fn dialog_button(accent: bool) -> String {
    let border = if accent {
        tokens::COLOR_MACRO_BADGE
    } else {
        tokens::COLOR_BORDER_CHROME
    };
    format!(
        "min-height: {th}px; padding: {pv}px {ph}px; background: transparent; \
         border: 1px solid {border}; border-radius: {r}px; color: {fg}; \
         font-size: {s}px; cursor: pointer;",
        th = tokens::TOUCH_MIN,
        pv = tokens::SPACE_1,
        ph = tokens::SPACE_3,
        border = border,
        r = tokens::RADIUS_SM,
        fg = tokens::COLOR_TEXT_ON_CHROME,
        s = tokens::FONT_SIZE_BODY,
    )
}
