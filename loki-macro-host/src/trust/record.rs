// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! [`TrustRecord`] — one document's local trust state (macro spec §2.4).

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::capability::{Capability, GrantScope};

/// The user's standing decision about a document's macros (spec §2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TrustDecision {
    /// Macros stay disabled; nothing executes. The default, and sticky when the
    /// user explicitly chose "Keep disabled" (so the notice collapses to a chip
    /// on later opens, spec §2.3).
    #[default]
    Disabled,
    /// Enabled until the document is closed. **Never persisted** — a record with
    /// this decision is session state only (held by `MacroService`, not the
    /// on-disk store).
    SessionOnly,
    /// Persistently trusted; explicitly-invoked macros run with the baseline
    /// capability set on every open (until the payload hash changes).
    Trusted,
}

impl TrustDecision {
    /// Whether this decision permits execution of explicitly-invoked macros.
    #[must_use]
    pub fn is_enabled(self) -> bool {
        matches!(self, TrustDecision::SessionOnly | TrustDecision::Trusted)
    }

    /// Whether a record with this decision belongs in the persistent store.
    /// `SessionOnly` is deliberately excluded (spec §2.3).
    #[must_use]
    pub fn is_persistent(self) -> bool {
        matches!(self, TrustDecision::Disabled | TrustDecision::Trusted)
    }
}

/// How a document's macros came to be trusted (spec §2.5).
///
/// Trust is never inferred from a document's own content (T10); this records
/// *how* an existing trust record was established, so an in-app edit can keep
/// trust while an external modification cannot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Provenance {
    /// The document arrived from elsewhere (file manager, download, sync, a
    /// collaboration server). The default — nothing self-authored.
    #[default]
    External,
    /// The macros were authored or edited **in Loki on this machine** (spec
    /// §2.5): created here, or last written back through the macro editor. Such
    /// an edit re-keys the trust record to the new payload hash rather than
    /// dropping trust (an external change to the same bytes still would, because
    /// it never travels through the editor's re-key path).
    AuthoredHere,
    /// The macros carry a valid signature from a **user-pinned trusted
    /// publisher** (ADR-0014 §4.5, 8A.5). The trust anchor is the signer
    /// certificate's SHA-256 thumbprint, recorded here so a later verify against
    /// the same publisher can be recognised; trust still comes from the pin in
    /// the `TrustedPublisherStore`, never from the document (T10).
    TrustedPublisher {
        /// The pinned signer-certificate thumbprint that matched.
        #[serde(with = "hex32")]
        thumbprint: [u8; 32],
    },
}

/// A persisted, always-for-document capability grant (spec §2.4).
///
/// Only [`GrantScope::AlwaysForDocument`] grants are recorded here; once/session
/// grants live in ephemeral session state and never reach disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedGrant {
    /// The granted capability.
    pub capability: Capability,
    /// The scope at which it was granted (always [`GrantScope::AlwaysForDocument`]
    /// for a persisted grant; stored explicitly for forward-compatibility).
    pub scope: GrantScope,
}

/// A document's local trust state, keyed by the **hash of its macro payload**
/// (spec §2.4). Nothing here is ever written into a document; nothing in a
/// document is ever read into one of these (threat T10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustRecord {
    /// Content hash of the canonicalised macro payload
    /// ([`loki_doc_model::io::macros::MacroPayload::payload_hash`]). Serialized
    /// as lowercase hex.
    #[serde(with = "hex32")]
    pub doc_key: [u8; 32],
    /// Advisory display path of where the document was last seen. **Never used
    /// for matching** — trust is keyed by payload hash alone (spec §2.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_path: Option<PathBuf>,
    /// The standing trust decision.
    pub decision: TrustDecision,
    /// Whether on-open / auto events may fire (a separate, scarier opt-in,
    /// spec §5.6). `false` unless the user explicitly enabled auto-run.
    #[serde(default)]
    pub auto_run_open: bool,
    /// Whether this document may attempt macro network access (ADR-0015 §8): the
    /// per-document half of the runtime setting that, ANDed with the `macro-net`
    /// build feature, unlocks the origin-scoped `Network` capability. `false`
    /// unless the user explicitly allowed it; per-origin prompts still apply.
    #[serde(default)]
    pub allow_network: bool,
    /// Persisted always-for-document capability grants (spec §5.4).
    #[serde(default)]
    pub capability_grants: Vec<PersistedGrant>,
    /// How this trust was established (spec §2.5). Defaults to
    /// [`Provenance::External`] for records written before this field existed.
    #[serde(default)]
    pub provenance: Provenance,
    /// Unix seconds when the record was created (advisory).
    #[serde(default)]
    pub created: u64,
    /// Unix seconds when the record was last consulted (advisory).
    #[serde(default)]
    pub last_used: u64,
}

impl TrustRecord {
    /// Creates a record for `doc_key` with the given decision, timestamped now.
    #[must_use]
    pub fn new(doc_key: [u8; 32], decision: TrustDecision) -> Self {
        let now = now_secs();
        Self {
            doc_key,
            origin_path: None,
            decision,
            auto_run_open: false,
            allow_network: false,
            capability_grants: Vec::new(),
            provenance: Provenance::External,
            created: now,
            last_used: now,
        }
    }

    /// Builder: attach an advisory origin path (display only).
    #[must_use]
    pub fn with_origin(mut self, path: Option<PathBuf>) -> Self {
        self.origin_path = path;
        self
    }

    /// Builder: set how this trust was established (spec §2.5).
    #[must_use]
    pub fn with_provenance(mut self, provenance: Provenance) -> Self {
        self.provenance = provenance;
        self
    }

    /// Whether these macros were authored/edited in Loki (spec §2.5).
    #[must_use]
    pub fn is_authored(&self) -> bool {
        self.provenance == Provenance::AuthoredHere
    }

    /// Whether an always-for-document allow grant exists for `cap`.
    #[must_use]
    pub fn grants(&self, cap: Capability) -> bool {
        self.capability_grants
            .iter()
            .any(|g| g.capability == cap && g.scope.is_allow())
    }

    /// Records an always-for-document grant for `cap`, replacing any prior
    /// grant for the same capability.
    pub fn set_grant(&mut self, cap: Capability, scope: GrantScope) {
        self.capability_grants.retain(|g| g.capability != cap);
        if scope.is_persistent() {
            self.capability_grants.push(PersistedGrant {
                capability: cap,
                scope,
            });
        }
        self.last_used = now_secs();
    }

    /// Removes any persisted grant for `cap` (immediate revocation, spec §9.4).
    pub fn revoke(&mut self, cap: Capability) {
        self.capability_grants.retain(|g| g.capability != cap);
        self.last_used = now_secs();
    }
}

/// Unix seconds now, or `0` if the clock is before the epoch (never, in
/// practice). Best-effort — timestamps are advisory only.
pub(crate) fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Serde adapter: `[u8; 32]` as a lowercase hex string, so the on-disk store is
/// human-readable and the hash is its own JSON object key elsewhere.
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
