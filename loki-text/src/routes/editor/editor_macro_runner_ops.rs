// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Orchestration + styling helpers for the macro runner panel
//! ([`super::editor_macro_runner`]), split out for the 300-line ceiling.
//!
//! [`start_run`] launches a run on a worker thread and drains its
//! prompts/dialogs on the UI thread; [`answer_prompt`] records a grant and
//! unblocks the worker; [`stop_run`] trips the cancel flag and unblocks any
//! outstanding prompt.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use appthere_ui::tokens;
use dioxus::prelude::*;
use futures_util::StreamExt;
use loki_i18n::fl;
use loki_macro_host::{AutoRunToken, Dialect, GrantScope, MacroRuntime, MacroService};

use super::editor_macro_bridge::{
    BridgeBackend, PendingPrompt, UiReply, UiRequest, prompt_channel,
};
use super::editor_macro_notice::{MacroCtx, MacroView, payload_of};
use super::editor_macro_run::{RunMessages, RunReport, apply_and_report, make_run_request};

/// One runnable procedure: which module it lives in and its name.
#[derive(Clone, PartialEq)]
pub(super) struct ProcEntry {
    pub(super) module: String,
    pub(super) source: String,
    pub(super) proc: String,
}

/// The runner panel's reactive state, bundled so run-launch takes one handle.
#[derive(Clone, Copy)]
pub(super) struct RunState {
    pub(super) report: Signal<Option<RunReport>>,
    pub(super) running: Signal<bool>,
    pub(super) pending: Signal<Option<PendingPrompt>>,
    pub(super) cancel: Signal<Option<Arc<AtomicBool>>>,
}

/// Starts a **user-invoked** run of `entry` (the normal path — no auto-run
/// token needed; the document is already enabled).
pub(super) fn start_run(
    ctx: &MacroCtx,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    svc: &MacroService,
    dialect: Dialect,
    entry: &ProcEntry,
    state: RunState,
) {
    launch(ctx, loro_doc, svc, dialect, entry, state, false);
}

/// Starts an **auto-run** of `entry` (an on-open handler). Fires only if the
/// document still authorizes auto-run (spec §5.6) — the same gate that mints the
/// [`AutoRunToken`] `run_event` requires, re-checked at fire time.
pub(super) fn start_auto_run(
    ctx: &MacroCtx,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    svc: &MacroService,
    dialect: Dialect,
    entry: &ProcEntry,
    state: RunState,
) {
    launch(ctx, loro_doc, svc, dialect, entry, state, true);
}

/// Launches a run on a worker thread, rendering prompts/dialogs and applying the
/// result on the UI thread when it finishes. `auto` fires an on-open event
/// handler through the token-gated `run_event`; it aborts silently if the
/// document no longer authorizes auto-run.
fn launch(
    ctx: &MacroCtx,
    loro_doc: Signal<Option<loro::LoroDoc>>,
    svc: &MacroService,
    dialect: Dialect,
    entry: &ProcEntry,
    state: RunState,
    auto: bool,
) {
    let RunState {
        mut report,
        mut running,
        mut pending,
        mut cancel,
    } = state;
    if running() {
        return;
    }
    let doc_state = ctx.0.clone();
    let Some(payload) = payload_of(&doc_state) else {
        if !auto {
            report.set(Some(RunReport::failed(fl!("macros-run-unreadable"))));
        }
        return;
    };
    // Auto-run must re-clear the gate at fire time (T1) — the token proves it.
    let token: Option<AutoRunToken> = if auto {
        match svc.authorize_auto_run(&payload) {
            Some(t) => Some(t),
            None => return, // no longer authorized — fire nothing
        }
    } else {
        None
    };
    let flag = Arc::new(AtomicBool::new(false));
    let request = make_run_request(&doc_state, svc, &payload, Arc::clone(&flag));
    let source = entry.source.clone();
    let proc = entry.proc.clone();

    let (req_tx, mut req_rx) = prompt_channel();
    let (result_tx, result_rx) = futures_channel::oneshot::channel();
    let worker_flag = Arc::clone(&flag);
    let spawned = std::thread::Builder::new()
        .name("loki-macro-run".into())
        .spawn(move || {
            let backend = BridgeBackend::new(req_tx, worker_flag);
            let outcome = match token {
                Some(tok) => {
                    MacroRuntime::run_event(&source, dialect, &proc, request, backend, &tok)
                }
                None => MacroRuntime::run(&source, dialect, &proc, request, backend),
            };
            let _keep = result_tx.send(outcome);
        });
    if spawned.is_err() {
        if !auto {
            report.set(Some(RunReport::failed(fl!("macros-run-unreadable"))));
        }
        return;
    }

    report.set(None);
    running.set(true);
    cancel.set(Some(flag));

    let messages = run_messages();
    spawn(async move {
        // Drain prompts until the worker's backend is dropped (run finished).
        while let Some(prompt) = req_rx.next().await {
            pending.set(Some(prompt));
        }
        let rep = match result_rx.await {
            Ok(outcome) => match loro_doc.read().as_ref() {
                Some(loro) => apply_and_report(&doc_state, loro, outcome, &messages),
                None => RunReport::failed(messages.unreadable.clone()),
            },
            Err(_) => RunReport::failed(messages.unreadable.clone()),
        };
        report.set(Some(rep));
        pending.set(None);
        cancel.set(None);
        running.set(false);
    });
}

/// Records a capability grant (so future runs remember) and answers the prompt,
/// unblocking the worker.
pub(super) fn answer_prompt(
    svc: &MacroService,
    payload: Option<&loki_doc_model::io::macros::MacroPayload>,
    mut pending: Signal<Option<PendingPrompt>>,
    reply: UiReply,
) {
    let cap = pending.read().as_ref().and_then(|p| match p.request() {
        UiRequest::Capability(c) => Some(*c),
        // A network grant is session-max and recorded by the broker's
        // per-origin path during the run (ADR-0015 §4.2), never as a persisted
        // capability grant here.
        // TODO(8B.5-session-origins): remember an AllowSession origin in the
        // MacroService (session memory, never disk) and fold it into each run's
        // NetworkPolicy so an already-allowed origin does not re-prompt per run.
        UiRequest::Network(_) | UiRequest::Dialog(_) => None,
    });
    if let (Some(cap), Some(payload), UiReply::Grant(scope)) = (cap, payload, &reply) {
        match scope {
            GrantScope::AllowSession => svc.grant_session(payload, cap),
            GrantScope::AlwaysForDocument => {
                if let Err(e) = svc.grant_always(payload, cap) {
                    tracing::warn!("macro grant save failed: {e}");
                }
            }
            _ => {}
        }
    }
    if let Some(p) = pending.write().take() {
        p.answer(reply);
    }
}

/// Stops the running macro: trips the cancel flag and unblocks any prompt the
/// worker is waiting on (spec §8).
pub(super) fn stop_run(
    cancel: Signal<Option<Arc<AtomicBool>>>,
    mut pending: Signal<Option<PendingPrompt>>,
) {
    if let Some(flag) = cancel.read().as_ref() {
        flag.store(true, Ordering::SeqCst);
    }
    if let Some(p) = pending.write().take() {
        p.deny();
    }
}

/// Collects the runnable procedures across all readable modules.
pub(super) fn collect_procs(view: &MacroView, dialect: Dialect) -> Vec<ProcEntry> {
    let mut out = Vec::new();
    for module in &view.modules {
        if let Ok(names) = MacroRuntime::list_procedures(&module.source, dialect) {
            for proc in names {
                out.push(ProcEntry {
                    module: module.name.clone(),
                    source: module.source.clone(),
                    proc,
                });
            }
        }
    }
    out
}

/// The runnable on-open auto-run handlers (`Document_Open`, `AutoOpen`, …)
/// among `view`'s modules — the candidates for auto-firing (spec §5.6).
pub(super) fn auto_open_entries(view: &MacroView, dialect: Dialect) -> Vec<ProcEntry> {
    collect_procs(view, dialect)
        .into_iter()
        .filter(|e| loki_macro_host::is_auto_open(&e.proc))
        .collect()
}

/// The runnable procedure named `name` (case-insensitive) among `view`'s modules
/// — the target of a MACROBUTTON click (spec §6).
pub(super) fn entry_by_name(view: &MacroView, dialect: Dialect, name: &str) -> Option<ProcEntry> {
    collect_procs(view, dialect)
        .into_iter()
        .find(|e| e.proc.eq_ignore_ascii_case(name))
}

/// Resolved i18n strings for run outcomes.
fn run_messages() -> RunMessages {
    RunMessages {
        done: fl!("macros-run-done"),
        done_edited: fl!("macros-run-done-edited"),
        refused: fl!("macros-run-refused"),
        denied: fl!("macros-run-denied"),
        stopped: fl!("macros-run-stopped"),
        unreadable: fl!("macros-run-unreadable"),
    }
}

/// Style for the run-result banner (accent by success).
pub(super) fn report_style(ok: bool) -> String {
    format!(
        "margin-top: 4px; padding: {p}px {p2}px; border-left: 3px solid {c}; \
         font-size: {s}px; color: {fg};",
        p = tokens::SPACE_1,
        p2 = tokens::SPACE_2,
        c = if ok {
            tokens::COLOR_TAB_ACTIVE_INDICATOR
        } else {
            tokens::COLOR_STATUS_ERROR_BORDER
        },
        s = tokens::FONT_SIZE_LABEL,
        fg = tokens::COLOR_TEXT_ON_CHROME,
    )
}

/// Pill-button style; `accent` uses the macro-badge border.
pub(super) fn btn_style(accent: bool) -> String {
    let border = if accent {
        tokens::COLOR_MACRO_BADGE
    } else {
        tokens::COLOR_BORDER_CHROME
    };
    format!(
        "min-height: {th}px; padding: {pv}px {ph}px; background: {bg}; \
         border: 1px solid {border}; border-radius: {r}px; color: {fg}; \
         font-size: {size}px; cursor: pointer; flex-shrink: 0;",
        th = tokens::TOUCH_MIN,
        pv = tokens::SPACE_1,
        ph = tokens::SPACE_2,
        bg = tokens::COLOR_SURFACE_3,
        border = border,
        r = tokens::RADIUS_SM,
        fg = tokens::COLOR_TEXT_ON_CHROME,
        size = tokens::FONT_SIZE_LABEL,
    )
}
