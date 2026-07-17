// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared, UI-thread-side helpers for the interactive macro runner (macro spec
//! §5, §6).
//!
//! The runner ([`super::editor_macro_runner`]) executes the interpreter on a
//! worker thread (so a long run never freezes the UI and Stop stays live) with
//! the [`super::editor_macro_bridge`] backend for live capability prompts and
//! dialogs. These helpers are the parts that must run on the UI thread — reading
//! the document body and applying the resulting [`super::editor_macro_apply`]
//! batch as **one undo entry** — plus the request/report plumbing. They are
//! Dioxus-free so they can be unit-tested against a bare `DocumentState`.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use loki_macro_host::{MacroRunError, MacroService, RunOutcome, RunRequest};

use crate::editing::state::DocumentState;

/// Fuel budget for an in-app run (spec §8). Generous for document automation,
/// but bounded so a runaway macro is always stopped.
pub(super) const RUN_FUEL: u64 = 5_000_000;

/// The result of a run, for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RunReport {
    /// Whether the macro finished cleanly.
    pub(super) ok: bool,
    /// The outcome message, already resolved to display text.
    pub(super) message: String,
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
            applied: false,
        }
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

/// Builds the [`RunRequest`] for a run of `payload`'s macros: the document
/// title + body (read side), the capabilities resolved from the trust record,
/// the fuel budget, and the shared `cancel` flag driving Stop.
pub(super) fn make_run_request(
    doc_state: &Arc<Mutex<DocumentState>>,
    svc: &MacroService,
    payload: &loki_doc_model::io::macros::MacroPayload,
    cancel: Arc<AtomicBool>,
) -> RunRequest {
    let (title, text) = read_document(doc_state);
    let grants = svc.grant_set_for(payload);
    RunRequest::new(title, text, RUN_FUEL)
        .with_grants(grants)
        .with_cancel(cancel)
}

/// Applies a finished run's edits (on the UI thread) and builds the report. On a
/// clean run with edits, the whole batch applies as one undo entry (spec §6.2).
pub(super) fn apply_and_report(
    doc_state: &Arc<Mutex<DocumentState>>,
    loro: &loro::LoroDoc,
    outcome: RunOutcome,
    messages: &RunMessages,
) -> RunReport {
    match outcome.result {
        Ok(()) => {
            let applied = if outcome.batch.is_empty() {
                false
            } else {
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
                applied,
            }
        }
        Err(err) => RunReport {
            ok: false,
            message: describe_error(&err, messages),
            applied: false,
        },
    }
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
