// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Document Security panel summaries produced by [`MacroService`] (macro spec
//! §9.4).

use loki_doc_model::io::macros::MacroPayload;

use super::MacroService;
use crate::capability::Capability;
use crate::trust::{TrustDecision, TrustRecord};

/// One capability's grant state for the Document Security panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityState {
    /// The capability this row describes.
    pub capability: Capability,
    /// Granted `AlwaysForDocument` (persisted).
    pub persisted: bool,
    /// Granted `AllowSession` (this session only).
    pub session: bool,
    /// Permanently refused (spec §7) — shown as such, never grantable.
    pub refused: bool,
}

impl CapabilityState {
    /// Whether the capability is currently granted at all.
    #[must_use]
    pub fn granted(&self) -> bool {
        self.persisted || self.session
    }
}

/// A snapshot of a document's macro-security state (spec §9.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSecurity {
    /// The payload hash (trust-store key).
    pub key: [u8; 32],
    /// The effective trust decision.
    pub decision: TrustDecision,
    /// Whether on-open auto-run is enabled (spec §5.6).
    pub auto_run_open: bool,
    /// Whether a persistent record exists for this document.
    pub has_record: bool,
    /// Per-capability grant state (baseline `DocRead` omitted).
    pub capabilities: Vec<CapabilityState>,
}

impl MacroService {
    /// A snapshot of the document's security state for the Document Security
    /// panel.
    #[must_use]
    pub fn security_for(&self, payload: &MacroPayload) -> DocumentSecurity {
        let key = payload.payload_hash();
        let inner = self.read();
        let session = inner.sessions.get(&key);
        let record = inner.store.get(&key);
        let decision = if session.is_some_and(|s| s.session_enabled) {
            TrustDecision::SessionOnly
        } else {
            record.map(|r| r.decision).unwrap_or_default()
        };
        let capabilities = Capability::ALL
            .iter()
            .filter(|c| !c.is_baseline())
            .map(|&cap| {
                let persisted = record.is_some_and(|r| r.grants(cap));
                let session_granted = session.is_some_and(|s| s.session_grants.contains(&cap));
                CapabilityState {
                    capability: cap,
                    persisted,
                    session: session_granted,
                    refused: cap.is_refused_in_v1(),
                }
            })
            .collect();
        DocumentSecurity {
            key,
            decision,
            auto_run_open: record.is_some_and(|r| r.auto_run_open),
            has_record: record.is_some(),
            capabilities,
        }
    }

    /// All persistent trust records, newest-used first, for the global
    /// management list (spec §9.4).
    #[must_use]
    pub fn all_records(&self) -> Vec<TrustRecord> {
        let mut recs: Vec<TrustRecord> = self.read().store.records().cloned().collect();
        recs.sort_by(|a, b| b.last_used.cmp(&a.last_used));
        recs
    }
}
