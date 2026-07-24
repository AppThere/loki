// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Capability-grant methods on [`MacroService`] (macro spec §5.4).

use loki_doc_model::io::macros::MacroPayload;

use super::MacroService;
use crate::broker::GrantSet;
use crate::capability::{Capability, GrantScope};
use crate::error::MacroHostError;
use crate::trust::{TrustDecision, TrustRecord};

impl MacroService {
    /// Grants `cap` for this session only (`AllowSession`). Refused capabilities
    /// are ignored.
    pub fn grant_session(&self, payload: &MacroPayload, cap: Capability) {
        if cap.is_refused_in_v1() {
            return;
        }
        let key = payload.payload_hash();
        self.write()
            .sessions
            .entry(key)
            .or_default()
            .session_grants
            .insert(cap);
    }

    /// Grants `cap` `AlwaysForDocument`, persisting it to the trust record. A
    /// persistent grant implies persistent trust, so the document is recorded
    /// [`TrustDecision::Trusted`] if it was not already. Refused capabilities
    /// are ignored.
    ///
    /// # Errors
    /// Propagates a trust-store save failure.
    pub fn grant_always(
        &self,
        payload: &MacroPayload,
        cap: Capability,
    ) -> Result<(), MacroHostError> {
        if cap.is_refused_in_v1() {
            return Ok(());
        }
        let key = payload.payload_hash();
        let mut inner = self.write();
        if inner.store.get(&key).is_none() {
            inner
                .store
                .insert(TrustRecord::new(key, TrustDecision::Trusted));
        }
        if let Some(rec) = inner.store.get_mut(&key) {
            if !rec.decision.is_enabled() {
                rec.decision = TrustDecision::Trusted;
            }
            rec.set_grant(cap, GrantScope::AlwaysForDocument);
        }
        inner.store.save()
    }

    /// Revokes `cap` immediately — removes both the persisted grant and any
    /// session grant (spec §9.4).
    ///
    /// # Errors
    /// Propagates a trust-store save failure.
    pub fn revoke(&self, payload: &MacroPayload, cap: Capability) -> Result<(), MacroHostError> {
        let key = payload.payload_hash();
        let mut inner = self.write();
        if let Some(sess) = inner.sessions.get_mut(&key) {
            sess.session_grants.remove(&cap);
        }
        if let Some(rec) = inner.store.get_mut(&key) {
            rec.revoke(cap);
        }
        inner.store.save()
    }

    /// The capabilities resolved for a run of `payload`: persisted
    /// always-grants plus this session's grants. Baseline `DocRead` is granted
    /// by the broker regardless and is not listed here.
    #[must_use]
    pub fn grant_set_for(&self, payload: &MacroPayload) -> GrantSet {
        let key = payload.payload_hash();
        let inner = self.read();
        let mut set = GrantSet::from_record(inner.store.get(&key));
        if let Some(sess) = inner.sessions.get(&key) {
            for &cap in &sess.session_grants {
                set.allow(cap);
            }
        }
        set
    }
}
