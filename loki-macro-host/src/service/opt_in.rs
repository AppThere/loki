// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! The per-document runtime opt-ins that sit *on top of* trust: on-open
//! auto-run (spec §5.6) and network access (ADR-0015 §8). Each is a separate,
//! scarier switch than plain enablement — off by default, requiring a persistent
//! trust record — and is split from `service/mod.rs` for the 300-line ceiling.

use loki_doc_model::io::macros::MacroPayload;

use super::MacroService;
use crate::error::MacroHostError;

impl MacroService {
    /// Whether on-open / auto events may fire for `payload` (spec §5.6). Only
    /// ever true for a persistently-trusted document with the explicit opt-in.
    #[must_use]
    pub fn auto_run_open(&self, payload: &MacroPayload) -> bool {
        self.read()
            .store
            .get(&payload.payload_hash())
            .is_some_and(|r| r.auto_run_open)
    }

    /// Returns an [`AutoRunToken`](crate::runtime::AutoRunToken) **iff** on-open
    /// events are authorized for `payload` — the document is persistently trusted
    /// *and* the user set the separate `auto_run_open` opt-in (spec §5.6). The
    /// token is the only key to [`crate::MacroRuntime::run_event`], so nothing can
    /// fire on open without this gate returning `Some` (threat T1). A session-only
    /// or disabled document (no persistent record) never authorizes auto-run.
    #[must_use]
    pub fn authorize_auto_run(
        &self,
        payload: &MacroPayload,
    ) -> Option<crate::runtime::AutoRunToken> {
        let inner = self.read();
        let rec = inner.store.get(&payload.payload_hash())?;
        (rec.decision.is_enabled() && rec.auto_run_open).then(crate::runtime::AutoRunToken::new)
    }

    /// Sets the auto-run-on-open opt-in (spec §5.6). Requires an existing
    /// persistent record; a no-op if the document is not trusted.
    ///
    /// # Errors
    /// Propagates a trust-store save failure.
    pub fn set_auto_run_open(
        &self,
        payload: &MacroPayload,
        enabled: bool,
    ) -> Result<(), MacroHostError> {
        let key = payload.payload_hash();
        let mut inner = self.write();
        if let Some(rec) = inner.store.get_mut(&key) {
            rec.auto_run_open = enabled;
        }
        inner.store.save()
    }

    /// Whether `payload` is allowed to attempt macro network access — the
    /// per-document half of the runtime setting (ADR-0015 §8). Only ever true for
    /// a document with a persistent record whose user set the opt-in. The caller
    /// must additionally check [`crate::MACRO_NET_COMPILED`]; per-origin prompts
    /// still gate every actual request.
    #[must_use]
    pub fn network_enabled(&self, payload: &MacroPayload) -> bool {
        self.read()
            .store
            .get(&payload.payload_hash())
            .is_some_and(|r| r.allow_network)
    }

    /// Sets the per-document network-access opt-in (ADR-0015 §8). Requires an
    /// existing persistent record; a no-op if the document is not trusted.
    ///
    /// # Errors
    /// Propagates a trust-store save failure.
    pub fn set_allow_network(
        &self,
        payload: &MacroPayload,
        enabled: bool,
    ) -> Result<(), MacroHostError> {
        let key = payload.payload_hash();
        let mut inner = self.write();
        if let Some(rec) = inner.store.get_mut(&key) {
            rec.allow_network = enabled;
        }
        inner.store.save()
    }
}
