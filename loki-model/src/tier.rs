// SPDX-License-Identifier: Apache-2.0

//! The tiered confidentiality model (ADR-C014) and its capability gates
//! (ADR-C015).

use serde::{Deserialize, Serialize};

/// Confidentiality tier of a workspace (default) or document (override).
///
/// | Tier | Server sees plaintext? | Server-side processing? | Key custody |
/// |------|------------------------|-------------------------|-------------|
/// | 0    | yes                    | yes                     | server KMS  |
/// | 1    | within trust boundary  | yes                     | customer KMS|
/// | 2    | never (ciphertext)     | no                      | client only |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EncryptionTier {
    /// Tier 0 — transport + at-rest encryption; server / platform KMS custody.
    TransportAtRest,
    /// Tier 1 — customer-managed keys (CMK); customer KMS/HSM custody.
    CustomerManagedKeys,
    /// Tier 2 — zero-knowledge E2EE; the server stores only ciphertext.
    ZeroKnowledge,
}

impl EncryptionTier {
    /// Whether the server ever handles document plaintext under this tier.
    #[must_use]
    pub const fn server_sees_plaintext(self) -> bool {
        !matches!(self, Self::ZeroKnowledge)
    }

    /// ADR-C015: server-side render/print/convert/search/thumbnail are
    /// feature-gated on `tier != 2`. Callers must return the typed
    /// `E2eeCapabilityDisabled` error when this is `false`.
    #[must_use]
    pub const fn allows_server_side_processing(self) -> bool {
        !matches!(self, Self::ZeroKnowledge)
    }

    /// Whether the server performs snapshot compaction (ADR-C013). Under
    /// Tier 2 the server compacts nothing; clients upload encrypted
    /// snapshots instead.
    #[must_use]
    pub const fn server_compacts_snapshots(self) -> bool {
        !matches!(self, Self::ZeroKnowledge)
    }

    /// Stable numeric form used in the database (`smallint`).
    #[must_use]
    pub const fn as_i16(self) -> i16 {
        match self {
            Self::TransportAtRest => 0,
            Self::CustomerManagedKeys => 1,
            Self::ZeroKnowledge => 2,
        }
    }
}

/// Error returned when a stored tier value is out of range.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid encryption tier {0} (expected 0, 1, or 2)")]
pub struct TierParseError(pub i16);

impl TryFrom<i16> for EncryptionTier {
    type Error = TierParseError;

    fn try_from(value: i16) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::TransportAtRest),
            1 => Ok(Self::CustomerManagedKeys),
            2 => Ok(Self::ZeroKnowledge),
            other => Err(TierParseError(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_round_trip() {
        for tier in [
            EncryptionTier::TransportAtRest,
            EncryptionTier::CustomerManagedKeys,
            EncryptionTier::ZeroKnowledge,
        ] {
            assert_eq!(EncryptionTier::try_from(tier.as_i16()), Ok(tier));
        }
        assert!(EncryptionTier::try_from(3).is_err());
    }

    #[test]
    fn tier2_disables_server_side_processing() {
        assert!(EncryptionTier::TransportAtRest.allows_server_side_processing());
        assert!(EncryptionTier::CustomerManagedKeys.allows_server_side_processing());
        assert!(!EncryptionTier::ZeroKnowledge.allows_server_side_processing());
        assert!(!EncryptionTier::ZeroKnowledge.server_sees_plaintext());
        assert!(!EncryptionTier::ZeroKnowledge.server_compacts_snapshots());
    }
}
