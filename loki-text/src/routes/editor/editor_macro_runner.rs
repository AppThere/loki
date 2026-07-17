// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The in-app macro runner panel (Tools ▸ Macros — macro spec §9.3).
//!
//! Lists an enabled document's runnable procedures and runs a chosen one on a
//! **worker thread** so a long or misbehaving run never freezes the UI. The
//! interpreter's first-use capability prompts (§5.4) and dialogs (§5.5) round-
//! trip to the UI thread through [`super::editor_macro_bridge`] and render in
//! the anti-spoof frame; an always-available **Stop** trips the run's cancel
//! flag (§8). On a clean finish the edits apply as **one undo entry**
//! ([`super::editor_macro_apply`]). Orchestration lives in
//! [`super::editor_macro_runner_ops`].

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use appthere_ui::tokens;
use dioxus::prelude::*;
use loki_i18n::fl;
use loki_macro_host::Dialect;

use super::editor_macro_bridge::{PendingPrompt, UiReply};
use super::editor_macro_notice::{MacroCtx, MacroView, payload_of};
use super::editor_macro_prompt::{MacroPromptView, PromptKind};
use super::editor_macro_run::RunReport;
use super::editor_macro_runner_ops::{
    RunState, answer_prompt, btn_style, collect_procs, report_style, start_run, stop_run,
};

/// The macro runner panel.
#[component]
pub(super) fn MacroRunnerPanel(
    ctx: MacroCtx,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    view: MacroView,
    dialect: Dialect,
    project: String,
    doc_title: String,
    on_close: EventHandler<()>,
) -> Element {
    let svc = use_context::<loki_macro_host::MacroService>();
    let state = RunState {
        report: use_signal(|| None::<RunReport>),
        running: use_signal(|| false),
        pending: use_signal(|| None::<PendingPrompt>),
        cancel: use_signal(|| None::<Arc<AtomicBool>>),
    };
    let RunState {
        report,
        running,
        pending,
        cancel,
    } = state;

    let procs = collect_procs(&view, dialect);
    let container = format!(
        "display: flex; flex-direction: column; gap: {gap}px; padding: {pv}px {ph}px; \
         background: {bg}; border-top: 1px solid {border}; border-bottom: 1px solid {border}; \
         font-family: {ff}; color: {fg}; flex-shrink: 0; max-height: 45vh; overflow-y: auto;",
        gap = tokens::SPACE_2,
        pv = tokens::SPACE_2,
        ph = tokens::SPACE_4,
        bg = tokens::COLOR_SURFACE_2,
        border = tokens::COLOR_BORDER_CHROME,
        ff = tokens::FONT_FAMILY_UI,
        fg = tokens::COLOR_TEXT_ON_CHROME,
    );

    rsx! {
        div { style: "{container}",
            // Header: title + Stop (while running) + Close.
            div {
                style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
                span {
                    style: format!("font-weight: bold; font-size: {}px;", tokens::FONT_SIZE_MD),
                    {fl!("macros-run-title")}
                }
                div { style: "flex: 1;" }
                if running() {
                    button {
                        style: btn_style(true),
                        onclick: move |_| stop_run(cancel, pending),
                        {fl!("macros-run-stop")}
                    }
                }
                button {
                    style: btn_style(false),
                    onclick: move |_| on_close.call(()),
                    {fl!("macros-run-close")}
                }
            }

            if procs.is_empty() {
                span {
                    style: format!("font-size: {}px; color: {};", tokens::FONT_SIZE_LABEL, tokens::COLOR_TEXT_ON_CHROME_SECONDARY),
                    {fl!("macros-run-none")}
                }
            }

            for entry in procs {
                {
                    let ctx_run = ctx.clone();
                    let svc_run = svc.clone();
                    let entry_run = entry.clone();
                    rsx! {
                        div {
                            key: "{entry.module}:{entry.proc}",
                            style: "display: flex; flex-direction: row; align-items: center; gap: 8px;",
                            span {
                                style: format!("flex: 1; font-size: {}px;", tokens::FONT_SIZE_LABEL),
                                "{entry.module} · {entry.proc}"
                            }
                            button {
                                style: btn_style(true),
                                disabled: running(),
                                onclick: move |_| start_run(&ctx_run, loro_doc, &svc_run, dialect, &entry_run, state),
                                {fl!("macros-run-action")}
                            }
                        }
                    }
                }
            }

            if let Some(rep) = report() {
                div { style: report_style(rep.ok), "{rep.message}" }
            }
        }

        // Live capability prompt / dialog from the running macro.
        if pending.read().is_some() {
            {
                let kind = pending
                    .read()
                    .as_ref()
                    .map(|p| PromptKind::from_request(p.request()))
                    .expect("pending is Some");
                let svc_answer = svc.clone();
                let payload_answer = payload_of(&ctx.0);
                rsx! {
                    MacroPromptView {
                        kind,
                        project: project.clone(),
                        doc_title: doc_title.clone(),
                        on_answer: move |reply: UiReply| {
                            answer_prompt(&svc_answer, payload_answer.as_ref(), pending, reply);
                        },
                    }
                }
            }
        }
    }
}
