// SPDX-License-Identifier: Apache-2.0

//! Request/response DTOs (spec §3). All JSON, all versioned under `/v1`.

use chrono::{DateTime, Utc};
use loki_crypto::WrappedDek;
use loki_model::{DocumentId, EncryptionTier, Role, UserId, WorkspaceId};
use loki_server_store::{DocMemberRecord, DocMetaRecord, UserRecord, WorkspaceRecord};
use serde::{Deserialize, Serialize};

/// `POST /v1/workspaces`
#[derive(Debug, Deserialize)]
pub struct CreateWorkspaceRequest {
    /// Workspace name (non-empty).
    pub name: String,
    /// Default tier for new documents; the deployment default when omitted
    /// (ratified decision §6.1).
    pub default_tier: Option<EncryptionTier>,
}

/// Workspace representation.
#[derive(Debug, Serialize)]
pub struct WorkspaceResponse {
    /// Identifier.
    pub id: WorkspaceId,
    /// Name.
    pub name: String,
    /// Default tier for new documents.
    pub default_tier: EncryptionTier,
    /// Data residency (ADR-C019).
    pub residency: String,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

impl From<WorkspaceRecord> for WorkspaceResponse {
    fn from(record: WorkspaceRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            default_tier: record.default_tier,
            residency: record.residency.as_config_value(),
            created_at: record.created_at,
        }
    }
}

/// `POST /v1/workspaces/{ws}/documents`
#[derive(Debug, Deserialize)]
pub struct CreateDocumentRequest {
    /// Document title (non-empty; server-visible metadata even under Tier 2).
    pub title: String,
    /// Tier override; the workspace default when omitted (ADR-C014).
    pub tier: Option<EncryptionTier>,
}

/// Document metadata representation. The wrapped DEK is deliberately not
/// exposed here; Tier-2 members receive their per-member wrap via the
/// membership listing.
#[derive(Debug, Serialize)]
pub struct DocumentResponse {
    /// Identifier.
    pub id: DocumentId,
    /// Owning workspace.
    pub workspace_id: WorkspaceId,
    /// Title.
    pub title: String,
    /// Confidentiality tier.
    pub tier: EncryptionTier,
    /// Data residency.
    pub residency: String,
    /// Whether a snapshot exists (`GET …/snapshot` succeeds).
    pub has_snapshot: bool,
    /// Highest oplog sequence the snapshot covers (`0` = none). After
    /// downloading the snapshot, connect the collab WebSocket with
    /// `after = snapshot_seq` to replay only the tail (ADR-C013).
    pub snapshot_seq: i64,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

impl From<DocMetaRecord> for DocumentResponse {
    fn from(record: DocMetaRecord) -> Self {
        Self {
            id: record.id,
            workspace_id: record.workspace_id,
            title: record.title,
            tier: record.tier,
            residency: record.residency.as_config_value(),
            has_snapshot: record.snapshot_ptr.is_some(),
            snapshot_seq: record.snapshot_seq,
            created_at: record.created_at,
        }
    }
}

/// `POST /v1/documents/{doc}/members`
#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    /// The user to grant a role to.
    pub user_id: UserId,
    /// The role (ADR-C017 matrix).
    pub role: Role,
    /// Tier 2 only: the document DEK wrapped to the new member's public key
    /// (the client-driven re-wrap surfaced by the API, ADR-C017).
    pub dek_wrapped_for_user: Option<WrappedDek>,
}

/// Membership representation.
#[derive(Debug, Serialize)]
pub struct MemberResponse {
    /// Member user id.
    pub user_id: UserId,
    /// Granted role.
    pub role: Role,
    /// This member's DEK wrap (present only under Tier 2, and only
    /// meaningful to that member's client).
    pub dek_wrapped_for_user: Option<WrappedDek>,
}

impl From<DocMemberRecord> for MemberResponse {
    fn from(record: DocMemberRecord) -> Self {
        Self {
            user_id: record.user_id,
            role: record.role,
            dek_wrapped_for_user: record.dek_wrapped_for_user,
        }
    }
}

/// `POST /v1/documents/{doc}/blobs`
#[derive(Debug, Serialize)]
pub struct BlobCreatedResponse {
    /// Object-storage key of the stored attachment.
    pub key: String,
}

/// `GET /v1/gdpr/export` — the caller's personal data (portability,
/// ADR-C020).
#[derive(Debug, Serialize)]
pub struct GdprExportResponse {
    /// Account id.
    pub user_id: UserId,
    /// OIDC subject.
    pub oidc_sub: String,
    /// Display name.
    pub display_name: String,
    /// Whether a Tier-2 public key is registered.
    pub has_public_key: bool,
}

impl From<UserRecord> for GdprExportResponse {
    fn from(record: UserRecord) -> Self {
        Self {
            user_id: record.id,
            oidc_sub: record.oidc_sub,
            display_name: record.display_name,
            has_public_key: record.public_key.is_some(),
        }
    }
}
