// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Signature state for the Document Security UI (8A.7; ADR-0014 §5).
//!
//! [`MacroService`] records the raw (pre-pin) [`SignatureVerdict`] a document
//! opened with, and resolves it against the pinned-publisher store on read, so a
//! fresh "Trust this publisher" pin flips the displayed state to trusted
//! immediately. [`SignatureSummary`] is the display-ready projection the panel
//! renders — coarse status plus the signer's common name, issuer, and thumbprint.

use loki_doc_model::io::macros::MacroPayload;
use loki_macro_sig::{SignatureVerdict, UntrustedReason};

use super::MacroService;
use crate::error::MacroHostError;
use crate::trust::PublisherRecord;

/// Coarse signature state for the security panel (ADR-0014 §5). Distinct from the
/// verifier's [`SignatureVerdict`]: it folds the `Invalid`/`ValidUntrusted`
/// reasons into the handful of states the UI shows differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureStatus {
    /// No macro signature present.
    Unsigned,
    /// A signature is present but broken — treated as unsigned, surfaced so the
    /// user is not misled.
    Invalid,
    /// Valid signature by a signer that is not (yet) a trusted publisher — the
    /// "Trust this publisher?" affordance applies.
    Untrusted,
    /// Valid, but signed only with a legacy (broken) algorithm Loki never trusts.
    Legacy,
    /// Valid, but the signing certificate has expired with no timestamp rescue.
    Expired,
    /// Same identity as a pinned publisher but a new thumbprint (certificate
    /// renewal) — the "re-pin the renewed certificate?" affordance applies.
    Renewed,
    /// Valid **and** the signer is a user-pinned trusted publisher.
    Trusted,
}

impl SignatureStatus {
    /// Whether any signature is present (intact or not).
    #[must_use]
    pub fn is_signed(self) -> bool {
        !matches!(self, SignatureStatus::Unsigned)
    }

    /// Whether the signer is a trusted publisher.
    #[must_use]
    pub fn is_trusted(self) -> bool {
        matches!(self, SignatureStatus::Trusted)
    }

    /// Whether the "Trust this publisher" / "re-pin renewed certificate"
    /// affordance should be offered. Legacy/expired signatures are **not**
    /// pinnable — pinning cannot make Loki trust a broken or expired signature.
    #[must_use]
    pub fn can_pin(self) -> bool {
        matches!(self, SignatureStatus::Untrusted | SignatureStatus::Renewed)
    }
}

/// Display-ready signature state for the Document Security panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureSummary {
    /// The coarse state.
    pub status: SignatureStatus,
    /// The signer common name (falling back to the full subject), if signed.
    pub signer_cn: Option<String>,
    /// The issuer distinguished name, if signed.
    pub issuer: Option<String>,
    /// The signer-certificate SHA-256 thumbprint, lowercase hex, if signed.
    pub thumbprint_hex: Option<String>,
}

impl SignatureSummary {
    /// The summary for a document with no recorded signature.
    #[must_use]
    pub fn unsigned() -> Self {
        Self {
            status: SignatureStatus::Unsigned,
            signer_cn: None,
            issuer: None,
            thumbprint_hex: None,
        }
    }

    /// Projects a resolved [`SignatureVerdict`] into display state.
    #[must_use]
    pub fn from_verdict(verdict: &SignatureVerdict) -> Self {
        let status = match verdict {
            SignatureVerdict::Unsigned => SignatureStatus::Unsigned,
            SignatureVerdict::Invalid(_) => SignatureStatus::Invalid,
            SignatureVerdict::ValidTrusted { .. } => SignatureStatus::Trusted,
            SignatureVerdict::ValidUntrusted { reason, .. } => match reason {
                UntrustedReason::NotPinned => SignatureStatus::Untrusted,
                UntrustedReason::LegacyAlgorithm => SignatureStatus::Legacy,
                UntrustedReason::CertificateExpired => SignatureStatus::Expired,
                UntrustedReason::PublisherRenewed => SignatureStatus::Renewed,
            },
        };
        let signer = verdict.signer();
        Self {
            status,
            signer_cn: signer.map(|c| {
                if c.subject_cn.is_empty() {
                    c.subject.clone()
                } else {
                    c.subject_cn.clone()
                }
            }),
            issuer: signer.map(|c| c.issuer.clone()),
            thumbprint_hex: signer.map(|c| c.thumbprint.to_hex()),
        }
    }
}

impl MacroService {
    /// Records the signature verdict a document opened with (called from the open
    /// path, 8A.8). Stored raw and resolved against the publisher store on read.
    pub fn set_signature(&self, payload: &MacroPayload, verdict: SignatureVerdict) {
        self.write()
            .signatures
            .insert(payload.payload_hash(), verdict);
    }

    /// The display summary for `payload`, resolving its recorded verdict against
    /// the pinned-publisher store. [`SignatureSummary::unsigned`] if none is
    /// recorded.
    #[must_use]
    pub fn signature_for(&self, payload: &MacroPayload) -> SignatureSummary {
        let inner = self.read();
        match inner.signatures.get(&payload.payload_hash()) {
            Some(verdict) => {
                let resolved = inner.publishers.resolve(verdict.clone());
                SignatureSummary::from_verdict(&resolved)
            }
            None => SignatureSummary::unsigned(),
        }
    }

    /// Whether `payload`'s signature resolves to a trusted publisher — the
    /// open-time "enabled at open" gate (ADR-0014 §4.5, wired in 8A.8).
    #[must_use]
    pub fn is_publisher_trusted(&self, payload: &MacroPayload) -> bool {
        let inner = self.read();
        inner
            .signatures
            .get(&payload.payload_hash())
            .is_some_and(|v| inner.publishers.resolve(v.clone()).is_trusted())
    }

    /// Pins the current document's signer as a trusted publisher — the "Trust this
    /// publisher" action (ADR-0014 §5). Returns `false` (pinning nothing) when the
    /// document has no verified signer. Persists.
    ///
    /// # Errors
    /// Propagates a publisher-store save failure (the pin still applies in memory).
    pub fn pin_publisher(&self, payload: &MacroPayload) -> Result<bool, MacroHostError> {
        let key = payload.payload_hash();
        let mut inner = self.write();
        let Some(signer) = inner.signatures.get(&key).and_then(|v| v.signer()).cloned() else {
            return Ok(false);
        };
        inner
            .publishers
            .pin(PublisherRecord::from_cert_info(&signer));
        inner.publishers.save()?;
        Ok(true)
    }

    /// Un-pins a publisher by thumbprint — the management-list remove and the
    /// local revocation mechanism (ADR-0014 §4.4). Persists.
    ///
    /// # Errors
    /// Propagates a publisher-store save failure.
    pub fn unpin_publisher(&self, thumbprint: &[u8; 32]) -> Result<(), MacroHostError> {
        let mut inner = self.write();
        inner.publishers.unpin(thumbprint);
        inner.publishers.save()
    }

    /// All pinned publishers for the management list, ordered by display name.
    #[must_use]
    pub fn trusted_publishers(&self) -> Vec<PublisherRecord> {
        let mut publishers: Vec<PublisherRecord> =
            self.read().publishers.records().cloned().collect();
        publishers.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        publishers
    }
}

#[cfg(test)]
#[path = "signature_tests.rs"]
mod tests;
