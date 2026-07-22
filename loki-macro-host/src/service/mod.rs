// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`MacroService`] — the suite-shared macro-security handle (macro spec §2, §5,
//! §9), provided into each app's context like `SpellService`.
//!
//! It wraps the persistent [`TrustStore`] plus **per-open-document session
//! state** (session-only trust and session capability grants, which never reach
//! disk). All trust decisions flow through here, so the rest of the app never
//! touches the store directly and can read the current state from any component.
//!
//! Cheap to clone (an `Arc` handle), so it drops straight into
//! `provide_context`. The impl is split across sibling modules: trust decisions
//! here, capability grants in [`grants`], UI summaries in [`summary`].

mod grants;
mod signature;
mod summary;

pub use signature::{SignatureStatus, SignatureSummary};
pub use summary::{CapabilityState, DocumentSecurity};

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use loki_doc_model::io::macros::MacroPayload;
use loki_macro_sig::SignatureVerdict;

use crate::capability::Capability;
use crate::error::MacroHostError;
use crate::trust::{TrustDecision, TrustRecord, TrustStore, TrustedPublisherStore};

/// Per-document, in-memory state that must not be persisted (spec §2.3, §5.4).
#[derive(Debug, Default)]
pub(super) struct SessionState {
    /// A session-only trust override (`Enable for this session`).
    pub(super) session_enabled: bool,
    /// Capabilities granted `AllowSession` this session.
    pub(super) session_grants: BTreeSet<Capability>,
}

pub(super) struct Inner {
    pub(super) store: TrustStore,
    /// The per-user pinned trusted-publisher store (ADR-0014 §4.3, 8A.5).
    pub(super) publishers: TrustedPublisherStore,
    pub(super) sessions: BTreeMap<[u8; 32], SessionState>,
    /// The raw (pre-pin) signature verdict recorded for each open document,
    /// keyed by payload hash. Resolved against `publishers` on read so pinning
    /// updates the displayed trust live (8A.7).
    pub(super) signatures: BTreeMap<[u8; 32], SignatureVerdict>,
}

/// Suite-shared macro-security service. See the module docs.
#[derive(Clone)]
pub struct MacroService {
    inner: Arc<RwLock<Inner>>,
}

impl MacroService {
    /// Boots the service, loading the trust store from `path` (or running purely
    /// in-memory when `None`). A corrupt store degrades to empty so a broken
    /// file never blocks opening a document.
    #[must_use]
    pub fn bootstrap(path: Option<PathBuf>) -> Self {
        // The pinned-publisher store lives beside the trust store in the same
        // profile directory (independent files, spec §4.5).
        let publisher_path = path.as_ref().map(|p| {
            p.parent().map_or_else(
                || PathBuf::from("trusted-publishers.json"),
                |dir| dir.join("trusted-publishers.json"),
            )
        });
        let store = match path {
            Some(p) => TrustStore::load_or_empty(p),
            None => TrustStore::default(),
        };
        let publishers = match publisher_path {
            Some(p) => TrustedPublisherStore::load_or_empty(p),
            None => TrustedPublisherStore::default(),
        };
        Self {
            inner: Arc::new(RwLock::new(Inner {
                store,
                publishers,
                sessions: BTreeMap::new(),
                signatures: BTreeMap::new(),
            })),
        }
    }

    /// An in-memory service with no persistence (tests, headless previews).
    #[must_use]
    pub fn in_memory() -> Self {
        Self::bootstrap(None)
    }

    pub(super) fn read(&self) -> RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn write(&self) -> RwLockWriteGuard<'_, Inner> {
        self.inner.write().unwrap_or_else(PoisonError::into_inner)
    }

    // ── Trust queries ─────────────────────────────────────────────────────────

    /// The effective decision for `payload`: a session override wins over the
    /// persistent record, which defaults to [`TrustDecision::Disabled`] when the
    /// document has no record (T10 — never inferred from the document).
    #[must_use]
    pub fn decision_for(&self, payload: &MacroPayload) -> TrustDecision {
        let key = payload.payload_hash();
        let inner = self.read();
        if inner.sessions.get(&key).is_some_and(|s| s.session_enabled) {
            return TrustDecision::SessionOnly;
        }
        inner.store.decision(&key)
    }

    /// Whether explicitly-invoked macros in `payload` may run.
    #[must_use]
    pub fn is_enabled(&self, payload: &MacroPayload) -> bool {
        self.decision_for(payload).is_enabled()
    }

    /// Whether on-open / auto events may fire for `payload` (spec §5.6). Only
    /// ever true for a persistently-trusted document with the explicit opt-in.
    #[must_use]
    pub fn auto_run_open(&self, payload: &MacroPayload) -> bool {
        self.read()
            .store
            .get(&payload.payload_hash())
            .is_some_and(|r| r.auto_run_open)
    }

    /// Returns an [`AutoRunToken`] **iff** on-open events are authorized for
    /// `payload` — the document is persistently trusted *and* the user set the
    /// separate `auto_run_open` opt-in (spec §5.6). The token is the only key to
    /// [`crate::MacroRuntime::run_event`], so nothing can fire on open without
    /// this gate returning `Some` (threat T1). A session-only or disabled
    /// document (no persistent record) never authorizes auto-run.
    #[must_use]
    pub fn authorize_auto_run(
        &self,
        payload: &MacroPayload,
    ) -> Option<crate::runtime::AutoRunToken> {
        let inner = self.read();
        let rec = inner.store.get(&payload.payload_hash())?;
        (rec.decision.is_enabled() && rec.auto_run_open).then(crate::runtime::AutoRunToken::new)
    }

    // ── Trust decisions (spec §2.3) ───────────────────────────────────────────

    /// Records the sticky "Keep disabled" choice (persisted so later opens show
    /// only the status chip). Clears any session enablement.
    ///
    /// # Errors
    /// Propagates a trust-store save failure (the choice still applies in
    /// memory).
    pub fn keep_disabled(
        &self,
        payload: &MacroPayload,
        origin: Option<PathBuf>,
    ) -> Result<(), MacroHostError> {
        let key = payload.payload_hash();
        let mut inner = self.write();
        inner.sessions.remove(&key);
        upsert_decision(&mut inner.store, key, TrustDecision::Disabled, origin);
        inner.store.save()
    }

    /// Enables macros for this session only (not persisted, spec §2.3).
    pub fn enable_session(&self, payload: &MacroPayload) {
        let key = payload.payload_hash();
        self.write()
            .sessions
            .entry(key)
            .or_default()
            .session_enabled = true;
    }

    /// Persistently trusts the document (spec §2.3).
    ///
    /// # Errors
    /// Propagates a trust-store save failure (trust still applies in memory).
    pub fn trust_document(
        &self,
        payload: &MacroPayload,
        origin: Option<PathBuf>,
    ) -> Result<(), MacroHostError> {
        let key = payload.payload_hash();
        let mut inner = self.write();
        upsert_decision(&mut inner.store, key, TrustDecision::Trusted, origin);
        inner.store.save()
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

    /// Re-keys trust after an **in-app macro edit** (spec §2.4/§2.5): the payload
    /// hash changes from `old`'s to `new`'s, so this carries the persistent
    /// record and any session override over to the new hash and marks the record
    /// self-authored ([`Provenance::AuthoredHere`]). A document with no prior
    /// trust gains none — trust is never fabricated. Only the macro editor's save
    /// path calls this; an external modification never does, so it still drops
    /// trust by plain hash mismatch.
    ///
    /// [`Provenance::AuthoredHere`]: crate::Provenance::AuthoredHere
    ///
    /// # Errors
    /// Propagates a trust-store save failure (the re-key still applies in
    /// memory).
    pub fn reauthor(&self, old: &MacroPayload, new: &MacroPayload) -> Result<(), MacroHostError> {
        let old_key = old.payload_hash();
        let new_key = new.payload_hash();
        let mut inner = self.write();
        // Carry any session-only override across to the new hash.
        if let Some(session) = inner.sessions.remove(&old_key) {
            inner.sessions.insert(new_key, session);
        }
        inner.store.reauthor(&old_key, new_key);
        inner.store.save()
    }

    /// Forgets the document entirely: removes the persistent record and session
    /// state (spec §9.4).
    ///
    /// # Errors
    /// Propagates a trust-store save failure.
    pub fn forget(&self, payload: &MacroPayload) -> Result<(), MacroHostError> {
        self.forget_key(&payload.payload_hash())
    }

    /// [`Self::forget`] by raw key — used by the management list, which holds
    /// keys rather than payloads.
    ///
    /// # Errors
    /// Propagates a trust-store save failure.
    pub fn forget_key(&self, key: &[u8; 32]) -> Result<(), MacroHostError> {
        let mut inner = self.write();
        inner.sessions.remove(key);
        inner.store.forget(key);
        inner.store.save()
    }

    /// Explicitly flushes the trust store to disk.
    ///
    /// # Errors
    /// Propagates a trust-store save failure.
    pub fn save(&self) -> Result<(), MacroHostError> {
        self.read().store.save()
    }
}

/// Inserts or updates the decision for `key`, preserving grants/auto-run on an
/// existing record and stamping the origin path (advisory).
pub(super) fn upsert_decision(
    store: &mut TrustStore,
    key: [u8; 32],
    decision: TrustDecision,
    origin: Option<PathBuf>,
) {
    if let Some(rec) = store.get_mut(&key) {
        rec.decision = decision;
        if origin.is_some() {
            rec.origin_path = origin;
        }
    } else {
        store.insert(TrustRecord::new(key, decision).with_origin(origin));
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
