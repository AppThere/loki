// SPDX-License-Identifier: Apache-2.0

//! Row types shared by all store implementations (spec §4).

use chrono::{DateTime, Utc};
use loki_crypto::WrappedDek;
use loki_model::{DocumentId, EncryptionTier, Residency, Role, UserId, WorkspaceId};

/// A workspace: the unit of membership and default policy.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceRecord {
    /// Workspace identifier.
    pub id: WorkspaceId,
    /// Human-readable name (workspace owners choose it; not localized).
    pub name: String,
    /// Default confidentiality tier for new documents (ADR-C014).
    pub default_tier: EncryptionTier,
    /// Data residency (ADR-C019).
    pub residency: Residency,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

/// A user account. Identity is delegated to OIDC (ADR-C017); no passwords.
#[derive(Debug, Clone, PartialEq)]
pub struct UserRecord {
    /// User identifier.
    pub id: UserId,
    /// OIDC subject (`iss`-scoped stable identifier).
    pub oidc_sub: String,
    /// Display name from the IdP.
    pub display_name: String,
    /// X25519 public key for Tier-2 DEK wrapping, once the client registers one.
    pub public_key: Option<Vec<u8>>,
}

/// Document metadata (`doc_meta`). Content lives in the oplog + snapshots.
#[derive(Debug, Clone, PartialEq)]
pub struct DocMetaRecord {
    /// Document identifier.
    pub id: DocumentId,
    /// Owning workspace.
    pub workspace_id: WorkspaceId,
    /// Title (plaintext even under Tier 2 — metadata is server-visible).
    pub title: String,
    /// Confidentiality tier (ADR-C014); gates server-side features (ADR-C015).
    pub tier: EncryptionTier,
    /// Data residency (ADR-C019).
    pub residency: Residency,
    /// Object-storage key of the current snapshot, if one exists.
    pub snapshot_ptr: Option<String>,
    /// The document DEK wrapped by the tier KEK (Tiers 0/1). `None` under
    /// Tier 2 (per-member wraps live on `doc_member`) and after
    /// crypto-shredding (ADR-C020).
    pub dek_wrapped: Option<WrappedDek>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

/// A document membership row (`doc_member`).
#[derive(Debug, Clone, PartialEq)]
pub struct DocMemberRecord {
    /// Document.
    pub doc_id: DocumentId,
    /// Member.
    pub user_id: UserId,
    /// RBAC role (ADR-C017).
    pub role: Role,
    /// Tier 2 only: the document DEK wrapped to this member's public key.
    pub dek_wrapped_for_user: Option<WrappedDek>,
}

/// One Loro update in the hot oplog (`doc_oplog`, ADR-C013).
///
/// `payload` is an opaque Loro update — ciphertext under Tier 2. The server
/// never interprets it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OplogEntry {
    /// Document.
    pub doc_id: DocumentId,
    /// Monotonic sequence within the table (orders updates per document).
    pub seq: i64,
    /// The member who produced the update.
    pub actor: UserId,
    /// Opaque Loro update bytes (AEAD ciphertext under Tier 2).
    pub payload: Vec<u8>,
    /// Ingest time.
    pub created_at: DateTime<Utc>,
}
