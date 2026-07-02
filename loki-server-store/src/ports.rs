// SPDX-License-Identifier: Apache-2.0

//! The persistence ports (traits) the rest of the server programs against.

use std::sync::Arc;

use async_trait::async_trait;
use loki_crypto::WrappedDek;
use loki_model::{DocumentId, EncryptionTier, Role, UserId, WorkspaceId};
use loki_server_audit::AuditEntry;

use crate::error::StoreError;
use crate::records::{DocMemberRecord, DocMetaRecord, OplogEntry, UserRecord, WorkspaceRecord};

/// Workspace CRUD.
#[async_trait]
pub trait WorkspaceStore: Send + Sync {
    /// Persists a new workspace.
    async fn create_workspace(&self, workspace: &WorkspaceRecord) -> Result<(), StoreError>;
    /// Loads a workspace by id.
    async fn get_workspace(&self, id: WorkspaceId) -> Result<Option<WorkspaceRecord>, StoreError>;
    /// Lists the documents in a workspace, newest first.
    async fn list_documents(&self, id: WorkspaceId) -> Result<Vec<DocMetaRecord>, StoreError>;
}

/// User accounts (OIDC-delegated identity, ADR-C017).
#[async_trait]
pub trait UserStore: Send + Sync {
    /// Finds a user by OIDC subject or provisions one just-in-time.
    async fn upsert_user_by_oidc(
        &self,
        oidc_sub: &str,
        display_name: &str,
    ) -> Result<UserRecord, StoreError>;
    /// Loads a user by id.
    async fn get_user(&self, id: UserId) -> Result<Option<UserRecord>, StoreError>;
    /// Registers the member's X25519 public key (Tier 2 sharing).
    async fn set_public_key(&self, id: UserId, public_key: &[u8]) -> Result<(), StoreError>;
    /// GDPR erasure: strips personal data (display name, public key) while
    /// keeping the row so foreign keys and the audit trail stay intact.
    async fn anonymize_user(&self, id: UserId) -> Result<(), StoreError>;
}

/// Document metadata (`doc_meta`).
#[async_trait]
pub trait DocumentStore: Send + Sync {
    /// Persists a new document.
    async fn create_document(&self, doc: &DocMetaRecord) -> Result<(), StoreError>;
    /// Loads document metadata.
    async fn get_document(&self, id: DocumentId) -> Result<Option<DocMetaRecord>, StoreError>;
    /// Points the document at a newly written snapshot covering every oplog
    /// entry with `seq <= up_to` (ADR-C013).
    ///
    /// Returns `false` (without changing anything) when the document already
    /// has a snapshot at `up_to` or newer — the guard that stops a slow
    /// concurrent compactor from regressing the pointer and orphaning
    /// truncated updates. Callers must only truncate the oplog on `true`.
    async fn set_snapshot(&self, id: DocumentId, ptr: &str, up_to: i64)
    -> Result<bool, StoreError>;
    /// Changes the confidentiality tier and replaces the wrapped DEK.
    async fn set_tier(
        &self,
        id: DocumentId,
        tier: EncryptionTier,
        dek_wrapped: Option<&WrappedDek>,
    ) -> Result<(), StoreError>;
    /// Deletes the document row (cascades members and oplog).
    async fn delete_document(&self, id: DocumentId) -> Result<(), StoreError>;
    /// Crypto-shreds the document: destroys every wrapped DEK copy
    /// (`doc_meta.dek_wrapped` and all `doc_member.dek_wrapped_for_user`),
    /// rendering Tier 1/2 ciphertext unrecoverable (ADR-C020).
    async fn shred_dek(&self, id: DocumentId) -> Result<(), StoreError>;
}

/// Document membership and roles.
#[async_trait]
pub trait MemberStore: Send + Sync {
    /// Grants or updates a member's role (plus Tier-2 DEK wrap when present).
    async fn upsert_member(&self, member: &DocMemberRecord) -> Result<(), StoreError>;
    /// Returns the member's role, if any.
    async fn get_member_role(
        &self,
        doc: DocumentId,
        user: UserId,
    ) -> Result<Option<Role>, StoreError>;
    /// Lists all members of a document.
    async fn list_members(&self, doc: DocumentId) -> Result<Vec<DocMemberRecord>, StoreError>;
    /// Revokes membership.
    async fn remove_member(&self, doc: DocumentId, user: UserId) -> Result<(), StoreError>;
}

/// The hot per-document oplog (ADR-C013).
#[async_trait]
pub trait OplogStore: Send + Sync {
    /// Appends an opaque update; returns its sequence number.
    async fn append(
        &self,
        doc: DocumentId,
        actor: UserId,
        payload: &[u8],
    ) -> Result<i64, StoreError>;
    /// Fetches updates with `seq > after`, oldest first.
    async fn fetch_after(&self, doc: DocumentId, after: i64)
    -> Result<Vec<OplogEntry>, StoreError>;
    /// Fetches one update by sequence number (used by the fan-out bus).
    async fn fetch_one(&self, doc: DocumentId, seq: i64) -> Result<Option<OplogEntry>, StoreError>;
    /// Drops updates with `seq <= up_to` after snapshot compaction.
    async fn truncate_up_to(&self, doc: DocumentId, up_to: i64) -> Result<(), StoreError>;
    /// Documents whose oplog holds at least `min_entries` updates, with the
    /// count — the compaction candidates (ADR-C013).
    async fn docs_with_backlog(
        &self,
        min_entries: i64,
    ) -> Result<Vec<(DocumentId, i64)>, StoreError>;
}

/// The append-only audit chain (ADR-C020).
///
/// Implementations must serialize appends so the hash chain stays linear
/// under concurrency.
#[async_trait]
pub trait AuditStore: Send + Sync {
    /// Appends an entry linked to the current chain head; returns it.
    async fn append_audit(
        &self,
        actor: &str,
        action: loki_server_audit::AuditAction,
        target: &str,
    ) -> Result<AuditEntry, StoreError>;
    /// Loads the full chain, oldest first (for verification/export).
    async fn load_chain(&self) -> Result<Vec<AuditEntry>, StoreError>;
}

/// Aggregate handed to API/collab layers: one field per port so tests can
/// mix production and in-memory implementations.
#[derive(Clone)]
pub struct Stores {
    /// Workspace port.
    pub workspaces: Arc<dyn WorkspaceStore>,
    /// User port.
    pub users: Arc<dyn UserStore>,
    /// Document-metadata port.
    pub documents: Arc<dyn DocumentStore>,
    /// Membership port.
    pub members: Arc<dyn MemberStore>,
    /// Oplog port.
    pub oplog: Arc<dyn OplogStore>,
    /// Audit port.
    pub audit: Arc<dyn AuditStore>,
}
