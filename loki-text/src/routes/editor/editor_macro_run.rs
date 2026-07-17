// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The synchronous, capability-pre-resolved macro runner (macro spec §5, §6,
//! Phase 5 — in-app).
//!
//! A run reads the document body, executes a **named** procedure against the
//! [`loki_macro_host`] execution engine using the capabilities the user has
//! already granted for the document (Document Security, §5.4), and applies the
//! resulting edits as **one undo entry** ([`super::editor_macro_apply`]).
//!
//! v1 posture (deliberate, so the runner ships without an un-verifiable async
//! UI): capabilities are **pre-resolved** from the trust record — the runner
//! does not prompt mid-run. A capability the macro needs but the user has not
//! granted surfaces as a trappable "permission denied", and the report tells the
//! user to grant it in Document Security and re-run. Macro-shown dialogs are
//! collected into an output log rather than rendered as blocking modals;
//! `InputBox` returns its default. Interactive first-use prompts, live modal
//! dialogs, and a worker-thread Stop are the follow-on (they need the async UI
//! round-trip). Execution is bounded by fuel (§8), so a synchronous run cannot
//! hang the UI.

use std::sync::{Arc, Mutex};

use loki_macro_host::{
    Dialect, DialogOutcome, MacroBackend, MacroRunError, MacroRuntime, MacroService, RunRequest,
};

use crate::editing::state::DocumentState;

/// Fuel budget for a synchronous in-app run (spec §8). Generous for document
/// automation, but bounded so the UI thread always returns.
const RUN_FUEL: u64 = 5_000_000;

/// The result of a run, for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RunReport {
    /// Whether the macro finished cleanly.
    pub(super) ok: bool,
    /// A short outcome message key-family already resolved to display text.
    pub(super) message: String,
    /// Lines the macro tried to show via `MsgBox`/`InputBox` (spec §5.5) —
    /// collected rather than shown as blocking modals in v1.
    pub(super) dialog_log: Vec<String>,
    /// Whether document edits were applied (one undo entry).
    pub(super) applied: bool,
}

impl RunReport {
    /// A failed report carrying just a message (e.g. the document or live CRDT
    /// was unavailable).
    pub(super) fn failed(message: String) -> Self {
        Self {
            ok: false,
            message,
            dialog_log: Vec::new(),
            applied: false,
        }
    }
}

/// A backend for the synchronous runner: it never prompts (v1 uses pre-resolved
/// grants only) and collects dialog text instead of showing modals.
pub(super) struct RunnerBackend {
    log: Arc<Mutex<Vec<String>>>,
}

impl RunnerBackend {
    fn new(log: Arc<Mutex<Vec<String>>>) -> Self {
        Self { log }
    }
}

impl MacroBackend for RunnerBackend {
    // `prompt_capability` intentionally uses the default (deny): v1 does not
    // prompt mid-run; the user pre-grants in Document Security.

    fn show_dialog(&mut self, req: &loki_macro_host::DialogRequest) -> DialogOutcome {
        if let Ok(mut log) = self.log.lock() {
            log.push(req.prompt.clone());
        }
        match req.kind {
            // Auto-acknowledge a message (vbOK); return the default for input.
            loki_macro_host::DialogKind::Message => DialogOutcome::Button(1),
            loki_macro_host::DialogKind::Input => {
                DialogOutcome::Text(req.default.clone().unwrap_or_default())
            }
        }
    }
}

/// Identifies which macro procedure to run (source, dialect, procedure name).
#[derive(Debug, Clone, Copy)]
pub(super) struct MacroCode<'a> {
    /// The module source to parse.
    pub(super) source: &'a str,
    /// The dialect (VBA / `StarBasic`).
    pub(super) dialect: Dialect,
    /// The procedure name to invoke.
    pub(super) proc: &'a str,
}

/// Runs `code.proc` against the live document, applying any edits as one undo
/// entry. `messages` maps each outcome to display text (so this stays
/// i18n-agnostic — the caller passes resolved strings).
pub(super) fn run_macro(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro: &loro::LoroDoc,
    svc: &MacroService,
    payload: &loki_doc_model::io::macros::MacroPayload,
    code: MacroCode<'_>,
    messages: &RunMessages,
) -> RunReport {
    let (title, text) = read_document(doc_state);
    let grants = svc.grant_set_for(payload);
    let log = Arc::new(Mutex::new(Vec::new()));
    let backend = RunnerBackend::new(Arc::clone(&log));

    let req = RunRequest::new(title, text, RUN_FUEL).with_grants(grants);
    let outcome = MacroRuntime::run(code.source, code.dialect, code.proc, req, backend);
    let dialog_log = log.lock().map(|l| l.clone()).unwrap_or_default();

    match outcome.result {
        Ok(()) => {
            let applied = if outcome.batch.is_empty() {
                false
            } else {
                // Apply the whole batch as one undo entry (spec §6.2).
                super::editor_macro_apply::apply_edit_batch(doc_state, loro, &outcome.batch)
                    .unwrap_or(false)
            };
            RunReport {
                ok: true,
                message: if applied {
                    messages.done_edited.clone()
                } else {
                    messages.done.clone()
                },
                dialog_log,
                applied,
            }
        }
        Err(err) => RunReport {
            ok: false,
            message: describe_error(&err, messages),
            dialog_log,
            applied: false,
        },
    }
}

/// Resolved display strings for run outcomes (i18n-agnostic runner core).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RunMessages {
    pub(super) done: String,
    pub(super) done_edited: String,
    pub(super) refused: String,
    pub(super) denied: String,
    pub(super) stopped: String,
    pub(super) unreadable: String,
}

fn describe_error(err: &MacroRunError, m: &RunMessages) -> String {
    if err.is_refusal() {
        m.refused.clone()
    } else if err.is_resource_stop() {
        m.stopped.clone()
    } else {
        match err {
            MacroRunError::Parse(_) => m.unreadable.clone(),
            MacroRunError::Runtime { number: 70, .. } => m.denied.clone(),
            MacroRunError::Runtime { message, .. } => message.clone(),
        }
    }
}

/// Reads the document title and section-0 plain text (blocks joined by `\n`)
/// from the published document — the macro's read-side view of the body.
fn read_document(doc_state: &Arc<Mutex<DocumentState>>) -> (String, String) {
    let Ok(state) = doc_state.lock() else {
        return (String::new(), String::new());
    };
    let Some(doc) = state.document.as_ref() else {
        return (String::new(), String::new());
    };
    let title = doc.meta.title.clone().unwrap_or_default();
    let text = doc
        .sections
        .first()
        .map(|s| {
            s.blocks
                .iter()
                .map(block_plain_text)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    (title, text)
}

/// The plain text of a block (paragraph runs concatenated); non-text blocks
/// contribute an empty line.
fn block_plain_text(block: &loki_doc_model::content::block::Block) -> String {
    use loki_doc_model::content::block::Block;
    use loki_doc_model::content::inline::Inline;
    let inlines: &[Inline] = match block {
        Block::Para(inlines) => inlines,
        Block::Heading(_, _, inlines) => inlines,
        _ => return String::new(),
    };
    inlines
        .iter()
        .filter_map(|i| match i {
            Inline::Str(s) => Some(s.as_str()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
#[path = "editor_macro_run_tests.rs"]
mod tests;
