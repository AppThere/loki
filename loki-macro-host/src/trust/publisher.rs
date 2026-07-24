// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`TrustedPublisherStore`] — the per-user registry of pinned macro-signing
//! publishers (ADR-0014 §4.3, 8A.5).
//!
//! A macro signature verified by `loki-macro-sig` proves **integrity +
//! authorship**, never trust — an attacker signs their own malware with their
//! own certificate. Trust is a separate, explicit user act: pinning a signer's
//! certificate **thumbprint** here. [`TrustedPublisherStore::resolve`] is the one
//! place a fully-valid signature becomes [`SignatureVerdict::ValidTrusted`], by
//! matching the signer thumbprint against this store. Nothing in a document can
//! write it (the same T10 rule as [`super::TrustStore`]).

use std::collections::BTreeMap;
use std::path::PathBuf;

use loki_macro_sig::{CertInfo, SignatureVerdict, UntrustedReason};
use serde::{Deserialize, Serialize};

use super::record::now_secs;
use crate::error::MacroHostError;

/// On-disk schema version.
const STORE_VERSION: u32 = 1;

/// One pinned publisher: the trusted thumbprint plus display/identity fields.
///
/// The `subject`/`issuer` are kept only so a **certificate renewal** (same
/// identity, new thumbprint) can be recognised and re-prompted rather than
/// silently trusted or silently broken (ADR-0014 §4.3). Trust is the thumbprint
/// alone — the blast radius is exactly one leaf certificate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublisherRecord {
    /// SHA-256 thumbprint of the signer's DER certificate — the trust anchor and
    /// the store key. Serialized as lowercase hex.
    #[serde(with = "hex32")]
    pub thumbprint: [u8; 32],
    /// Human-friendly label for the management UI (the signer common name).
    pub display_name: String,
    /// Full subject distinguished name (for renewal detection; display only).
    #[serde(default)]
    pub subject: String,
    /// Issuer distinguished name (for renewal detection; display only).
    #[serde(default)]
    pub issuer: String,
    /// Unix seconds when the user pinned this publisher (advisory).
    #[serde(default)]
    pub added: u64,
}

impl PublisherRecord {
    /// Builds a record from a verified signer certificate, timestamped now. The
    /// display name is the subject common name (falling back to the full
    /// subject).
    #[must_use]
    pub fn from_cert_info(info: &CertInfo) -> Self {
        let display_name = if info.subject_cn.is_empty() {
            info.subject.clone()
        } else {
            info.subject_cn.clone()
        };
        Self {
            thumbprint: *info.thumbprint.as_bytes(),
            display_name,
            subject: info.subject.clone(),
            issuer: info.issuer.clone(),
            added: now_secs(),
        }
    }
}

/// The serialized store: a version tag plus records keyed by hex thumbprint.
#[derive(Debug, Serialize, Deserialize, Default)]
struct StoreFile {
    version: u32,
    #[serde(default)]
    publishers: BTreeMap<String, PublisherRecord>,
}

/// The in-memory pinned-publisher store with optional on-disk backing.
#[derive(Debug, Default)]
pub struct TrustedPublisherStore {
    records: BTreeMap<[u8; 32], PublisherRecord>,
    /// Where the store persists (`None` = in-memory only, e.g. tests).
    path: Option<PathBuf>,
}

impl TrustedPublisherStore {
    /// An empty store that persists to `path` (or stays in-memory when `None`).
    #[must_use]
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            records: BTreeMap::new(),
            path,
        }
    }

    /// Loads the store from `path`; an absent file yields an empty store bound to
    /// it.
    ///
    /// # Errors
    ///
    /// [`MacroHostError::Io`] on a read error other than "not found";
    /// [`MacroHostError::Corrupt`] if the file cannot be parsed.
    pub fn load(path: PathBuf) -> Result<Self, MacroHostError> {
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::new(Some(path)));
            }
            Err(source) => return Err(MacroHostError::Io { path, source }),
        };
        let parsed: StoreFile =
            serde_json::from_str(&text).map_err(|e| MacroHostError::Corrupt {
                path: path.clone(),
                reason: e.to_string(),
            })?;
        let mut records = BTreeMap::new();
        for rec in parsed.publishers.into_values() {
            records.insert(rec.thumbprint, rec);
        }
        Ok(Self {
            records,
            path: Some(path),
        })
    }

    /// [`Self::load`] but a corrupt or unreadable file degrades to an empty store
    /// (still bound to `path`). Preferred at app startup.
    #[must_use]
    pub fn load_or_empty(path: PathBuf) -> Self {
        Self::load(path.clone()).unwrap_or_else(|_| Self::new(Some(path)))
    }

    /// Whether `thumbprint` is a pinned publisher.
    #[must_use]
    pub fn contains(&self, thumbprint: &[u8; 32]) -> bool {
        self.records.contains_key(thumbprint)
    }

    /// Pins a publisher (inserting or replacing the record for its thumbprint).
    pub fn pin(&mut self, record: PublisherRecord) {
        self.records.insert(record.thumbprint, record);
    }

    /// Un-pins a publisher — the local revocation mechanism (ADR-0014 §4.4).
    /// Returns the removed record, if any.
    pub fn unpin(&mut self, thumbprint: &[u8; 32]) -> Option<PublisherRecord> {
        self.records.remove(thumbprint)
    }

    /// A pinned publisher with the same subject **and** issuer as `info` but a
    /// *different* thumbprint — i.e. a certificate renewal (ADR-0014 §4.3).
    #[must_use]
    pub fn renewed_match(&self, info: &CertInfo) -> Option<&PublisherRecord> {
        let thumbprint = *info.thumbprint.as_bytes();
        self.records.values().find(|r| {
            r.thumbprint != thumbprint
                && !r.subject.is_empty()
                && r.subject == info.subject
                && r.issuer == info.issuer
        })
    }

    /// Upgrades a signature verdict to [`SignatureVerdict::ValidTrusted`] when the
    /// signer is pinned. Only a fully-valid, non-legacy, non-expired signature
    /// ([`UntrustedReason::NotPinned`]) is eligible: a legacy or expired signature
    /// stays untrusted regardless of pinning (downgrade/expiry defence, §4.2/§4.4).
    /// A renewal (pinned identity, new thumbprint) becomes
    /// [`UntrustedReason::PublisherRenewed`] so the UI can offer to re-pin.
    /// Every other verdict is returned unchanged.
    #[must_use]
    pub fn resolve(&self, verdict: SignatureVerdict) -> SignatureVerdict {
        let SignatureVerdict::ValidUntrusted { signer, reason } = &verdict else {
            return verdict;
        };
        if *reason != UntrustedReason::NotPinned {
            return verdict;
        }
        if self.contains(signer.thumbprint.as_bytes()) {
            return SignatureVerdict::ValidTrusted {
                thumbprint: signer.thumbprint,
                signer: signer.clone(),
            };
        }
        if self.renewed_match(signer).is_some() {
            return SignatureVerdict::ValidUntrusted {
                signer: signer.clone(),
                reason: UntrustedReason::PublisherRenewed,
            };
        }
        verdict
    }

    /// All pinned publishers, for the management list. Ordered by thumbprint.
    pub fn records(&self) -> impl Iterator<Item = &PublisherRecord> {
        self.records.values()
    }

    /// Number of pinned publishers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether no publisher is pinned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The configured persistence path, if any.
    #[must_use]
    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    /// Writes the pinned publishers to disk (creating parent directories). A
    /// store with no path is a no-op success (in-memory mode).
    ///
    /// # Errors
    ///
    /// [`MacroHostError::Io`] if the directory or file cannot be written, or the
    /// records cannot be serialized.
    pub fn save(&self) -> Result<(), MacroHostError> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let mut file = StoreFile {
            version: STORE_VERSION,
            publishers: BTreeMap::new(),
        };
        for rec in self.records.values() {
            file.publishers
                .insert(super::hex::encode(&rec.thumbprint), rec.clone());
        }
        let json = serde_json::to_string_pretty(&file).map_err(|e| MacroHostError::Io {
            path: path.clone(),
            source: std::io::Error::other(e),
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| MacroHostError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        std::fs::write(path, json).map_err(|source| MacroHostError::Io {
            path: path.clone(),
            source,
        })
    }
}

/// Serde adapter: `[u8; 32]` as a lowercase hex string.
mod hex32 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(bytes: &[u8; 32], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&super::super::hex::encode(bytes))
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<[u8; 32], D::Error> {
        let s = String::deserialize(d)?;
        super::super::hex::decode32(&s)
            .ok_or_else(|| serde::de::Error::custom("expected 64-character lowercase hex string"))
    }
}

#[cfg(test)]
#[path = "publisher_tests.rs"]
mod tests;
