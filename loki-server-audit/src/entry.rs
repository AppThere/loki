// SPDX-License-Identifier: Apache-2.0

//! Chain entries, hashing, and verification.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::action::AuditAction;

/// Byte length of the SHA-256 chain hashes.
pub const HASH_LEN: usize = 32;

/// The all-zero previous hash of the first entry in a chain.
const GENESIS_PREV_HASH: [u8; HASH_LEN] = [0u8; HASH_LEN];

/// One tamper-evident audit record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Position in the chain, starting at 1 (matches the DB identity column).
    pub seq: u64,
    /// Hash of the previous entry (all zeroes for the first entry).
    pub prev_hash: [u8; HASH_LEN],
    /// Hash of this entry's canonical encoding (including `prev_hash`).
    pub hash: [u8; HASH_LEN],
    /// Who acted — an OIDC subject or `"system"`.
    pub actor: String,
    /// What happened.
    pub action: AuditAction,
    /// What it happened to — a document/workspace/user id or a description.
    pub target: String,
    /// When it happened.
    pub created_at: DateTime<Utc>,
}

impl AuditEntry {
    /// Appends a new entry after `prev` (or starts a chain when `None`).
    #[must_use]
    pub fn append(
        prev: Option<&AuditEntry>,
        actor: impl Into<String>,
        action: AuditAction,
        target: impl Into<String>,
        created_at: DateTime<Utc>,
    ) -> Self {
        let (seq, prev_hash) = match prev {
            Some(p) => (p.seq + 1, p.hash),
            None => (1, GENESIS_PREV_HASH),
        };
        let actor = actor.into();
        let target = target.into();
        let hash = compute_hash(seq, &prev_hash, &actor, action, &target, created_at);
        Self {
            seq,
            prev_hash,
            hash,
            actor,
            action,
            target,
            created_at,
        }
    }

    /// Recomputes this entry's hash from its fields.
    #[must_use]
    pub fn expected_hash(&self) -> [u8; HASH_LEN] {
        compute_hash(
            self.seq,
            &self.prev_hash,
            &self.actor,
            self.action,
            &self.target,
            self.created_at,
        )
    }
}

/// Canonical hash: every variable-length field is length-prefixed (u64 BE)
/// so field boundaries cannot be shifted to forge a colliding encoding.
fn compute_hash(
    seq: u64,
    prev_hash: &[u8; HASH_LEN],
    actor: &str,
    action: AuditAction,
    target: &str,
    created_at: DateTime<Utc>,
) -> [u8; HASH_LEN] {
    let mut hasher = Sha256::new();
    hasher.update(b"loki-audit.v1");
    hasher.update(seq.to_be_bytes());
    hasher.update(prev_hash);
    for field in [actor, action.as_str(), target] {
        hasher.update((field.len() as u64).to_be_bytes());
        hasher.update(field.as_bytes());
    }
    hasher.update(created_at.timestamp_micros().to_be_bytes());
    hasher.finalize().into()
}

/// Why a chain failed verification.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChainError {
    /// An entry's stored hash does not match its recomputed hash.
    #[error("entry {seq} was mutated: stored hash does not match its content")]
    HashMismatch {
        /// Sequence number of the offending entry.
        seq: u64,
    },
    /// An entry's `prev_hash` does not match the previous entry's hash.
    #[error("chain broken at entry {seq}: prev_hash does not match entry {}", seq - 1)]
    BrokenLink {
        /// Sequence number of the offending entry.
        seq: u64,
    },
    /// Sequence numbers are not contiguous (an entry was inserted/removed).
    #[error("entry {seq} out of sequence (expected {expected})")]
    OutOfSequence {
        /// Sequence number found.
        seq: u64,
        /// Sequence number expected at this position.
        expected: u64,
    },
    /// The first entry does not start the chain correctly.
    #[error("first entry must have seq 1 and an all-zero prev_hash")]
    BadGenesis,
}

/// Verifies a contiguous slice of the chain (typically the whole table).
///
/// The slice must start at the genesis entry; verifying a tail requires the
/// caller to supply entries from `seq == 1`. An empty slice is valid.
pub fn verify_chain(entries: &[AuditEntry]) -> Result<(), ChainError> {
    let Some(first) = entries.first() else {
        return Ok(());
    };
    if first.seq != 1 || first.prev_hash != GENESIS_PREV_HASH {
        return Err(ChainError::BadGenesis);
    }
    let mut prev: Option<&AuditEntry> = None;
    for entry in entries {
        if let Some(p) = prev {
            if entry.seq != p.seq + 1 {
                return Err(ChainError::OutOfSequence {
                    seq: entry.seq,
                    expected: p.seq + 1,
                });
            }
            if entry.prev_hash != p.hash {
                return Err(ChainError::BrokenLink { seq: entry.seq });
            }
        }
        if entry.hash != entry.expected_hash() {
            return Err(ChainError::HashMismatch { seq: entry.seq });
        }
        prev = Some(entry);
    }
    Ok(())
}

#[cfg(test)]
#[path = "entry_tests.rs"]
mod tests;
