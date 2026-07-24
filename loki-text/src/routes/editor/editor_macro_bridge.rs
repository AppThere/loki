// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The worker↔UI bridge for interactive macro runs (macro spec §5.4, §5.5).
//!
//! A macro runs on a worker thread so a long or misbehaving run never freezes
//! the UI (the always-available **Stop** control lives on the UI thread). But
//! the interpreter's capability prompts ([`MacroBackend::prompt_capability`])
//! and dialogs ([`MacroBackend::show_dialog`]) are **synchronous** — the worker
//! must block until the user answers. This bridge carries each request to the UI
//! thread and blocks the worker on the reply:
//!
//! - **worker → UI:** a `futures` unbounded channel of [`PendingPrompt`]. The UI
//!   task awaits it with `StreamExt::next` and renders the matching component.
//! - **UI → worker:** each [`PendingPrompt`] carries a `std::sync::mpsc` reply
//!   sender; the worker blocks on the paired receiver (native blocking `recv`,
//!   no async executor needed on the worker).
//!
//! Requests are strictly sequential (the interpreter is single-threaded, so at
//! most one prompt is outstanding). **Stop / cancel:** the shared cancel flag is
//! checked before each request; if the worker is already blocked on a reply, the
//! UI answers the outstanding prompt with a deny/cancel so the worker unblocks
//! and the next fuel step aborts (spec §8). A dropped channel (runner closed)
//! likewise degrades to deny/cancel — a run can never wedge waiting on a UI that
//! is gone.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};

use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use loki_macro_host::{Capability, DialogOutcome, DialogRequest, GrantScope, MacroBackend};
#[cfg(feature = "macro-net")]
use loki_macro_host::{HttpError, HttpRequest, HttpResponse};

/// A request from the running macro that needs a UI answer.
pub(super) enum UiRequest {
    /// A first-use capability prompt (spec §5.4) — answer with a [`GrantScope`].
    Capability(Capability),
    /// A first-request-per-origin network prompt (ADR-0015 §4.2) carrying the
    /// destination origin — answer with a session-max [`GrantScope`].
    Network(String),
    /// A macro-shown dialog (spec §5.5) — answer with a [`DialogOutcome`].
    Dialog(DialogRequest),
}

/// The UI's answer to a [`UiRequest`].
pub(super) enum UiReply {
    /// Answer to a capability prompt.
    Grant(GrantScope),
    /// Answer to a dialog.
    Dialog(DialogOutcome),
}

/// A pending request plus the channel to answer it on.
pub(super) struct PendingPrompt {
    request: UiRequest,
    reply: Sender<UiReply>,
}

impl PendingPrompt {
    /// The request to render.
    pub(super) fn request(&self) -> &UiRequest {
        &self.request
    }

    /// Answers the prompt, unblocking the worker. A closed worker (already gone)
    /// is a no-op.
    pub(super) fn answer(self, reply: UiReply) {
        if self.reply.send(reply).is_err() {
            // The worker thread already finished/aborted — nothing to unblock.
        }
    }

    /// Answers a capability prompt (convenience).
    pub(super) fn deny(self) {
        self.answer(UiReply::Grant(GrantScope::Deny));
    }
}

/// Sender/receiver aliases for the worker→UI prompt channel.
pub(super) type PromptSender = UnboundedSender<PendingPrompt>;
pub(super) type PromptReceiver = UnboundedReceiver<PendingPrompt>;

/// Creates the worker→UI prompt channel.
pub(super) fn prompt_channel() -> (PromptSender, PromptReceiver) {
    unbounded()
}

/// The worker-side [`MacroBackend`]: it forwards prompts/dialogs to the UI and
/// blocks on the reply. Cancel-aware so Stop takes effect promptly.
pub(super) struct BridgeBackend {
    req_tx: PromptSender,
    cancel: Arc<AtomicBool>,
    /// The HTTPS fetcher, built lazily on first `HttpGet` (ADR-0015 §4.1, 8B.5).
    /// Only present when the `macro-net` feature is compiled in.
    #[cfg(feature = "macro-net")]
    fetcher: Option<loki_macro_host::NetFetcher>,
}

impl BridgeBackend {
    /// Creates a backend that posts to `req_tx` and honours `cancel`.
    pub(super) fn new(req_tx: PromptSender, cancel: Arc<AtomicBool>) -> Self {
        Self {
            req_tx,
            cancel,
            #[cfg(feature = "macro-net")]
            fetcher: None,
        }
    }

    /// Sends `request` to the UI and blocks for the reply, mapping a cancel or a
    /// gone-UI to `None`.
    fn ask(&self, request: UiRequest) -> Option<UiReply> {
        if self.cancel.load(Ordering::SeqCst) {
            return None;
        }
        let (reply_tx, reply_rx) = channel();
        let pending = PendingPrompt {
            request,
            reply: reply_tx,
        };
        if self.req_tx.unbounded_send(pending).is_err() {
            return None; // UI gone
        }
        reply_rx.recv().ok()
    }
}

impl MacroBackend for BridgeBackend {
    fn prompt_capability(&mut self, cap: Capability) -> GrantScope {
        match self.ask(UiRequest::Capability(cap)) {
            Some(UiReply::Grant(scope)) => scope,
            _ => GrantScope::Deny,
        }
    }

    fn prompt_network(&mut self, origin: &str) -> GrantScope {
        // The UI clamps the offered choices to session-max; a `Deny` (or a gone
        // UI / cancel) is the safe default.
        match self.ask(UiRequest::Network(origin.to_owned())) {
            Some(UiReply::Grant(scope)) => scope,
            _ => GrantScope::Deny,
        }
    }

    fn show_dialog(&mut self, req: &DialogRequest) -> DialogOutcome {
        match self.ask(UiRequest::Dialog(req.clone())) {
            Some(UiReply::Dialog(outcome)) => outcome,
            _ => DialogOutcome::Cancelled,
        }
    }

    /// Performs the gated HTTPS GET on the worker thread via the `reqwest`
    /// fetcher, passing the shared cancel flag so **Stop** aborts it (8B.5).
    /// Only compiled with `macro-net`; without it the trait default refuses.
    #[cfg(feature = "macro-net")]
    fn http_get(
        &mut self,
        request: &HttpRequest,
        allowed: &std::collections::BTreeSet<String>,
    ) -> Result<HttpResponse, HttpError> {
        if self.fetcher.is_none() {
            self.fetcher = Some(loki_macro_host::NetFetcher::new()?);
        }
        let Some(fetcher) = self.fetcher.as_ref() else {
            return Err(HttpError::Transport("fetcher unavailable".to_owned()));
        };
        fetcher.fetch(request, allowed, &self.cancel)
    }
}

#[cfg(test)]
#[path = "editor_macro_bridge_tests.rs"]
mod tests;
