// SPDX-License-Identifier: Apache-2.0

//! Document creation, metadata, listing, and snapshot download.

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::{Extension, Json};
use chrono::Utc;
use loki_crypto::Dek;
use loki_model::{Action, DocumentId, EncryptionTier, Role, WorkspaceId};
use loki_server_audit::AuditAction;
use loki_server_store::{DocMemberRecord, DocMetaRecord};

use crate::auth_mw::CurrentUser;
use crate::dto::{CreateDocumentRequest, DocumentResponse};
use crate::error::ApiError;
use crate::routes::require_doc_role;
use crate::state::ApiState;

pub(crate) async fn create(
    State(state): State<ApiState>,
    Path(ws): Path<WorkspaceId>,
    Extension(CurrentUser(user)): Extension<CurrentUser>,
    Json(request): Json<CreateDocumentRequest>,
) -> Result<(StatusCode, Json<DocumentResponse>), ApiError> {
    if request.title.trim().is_empty() {
        return Err(ApiError::Validation(
            "document title must not be empty".into(),
        ));
    }
    let workspace = state
        .stores
        .workspaces
        .get_workspace(ws)
        .await?
        .ok_or(ApiError::NotFound)?;
    let tier = request.tier.unwrap_or(workspace.default_tier);
    // Tier 0/1: the server generates the DEK and wraps it under the
    // deployment KEK. Tier 2: the DEK is client-held — the creator's client
    // registers per-member wraps through the members endpoint (ADR-C014).
    let dek_wrapped = if tier == EncryptionTier::ZeroKnowledge {
        None
    } else {
        Some(state.tier_kek.wrap(&Dek::generate())?)
    };
    let doc = DocMetaRecord {
        id: DocumentId::new(),
        workspace_id: ws,
        title: request.title,
        tier,
        residency: workspace.residency.clone(),
        snapshot_ptr: None,
        snapshot_seq: 0,
        dek_wrapped,
        created_at: Utc::now(),
    };
    state.stores.documents.create_document(&doc).await?;
    state
        .stores
        .members
        .upsert_member(&DocMemberRecord {
            doc_id: doc.id,
            user_id: user.id,
            role: Role::Owner,
            dek_wrapped_for_user: None,
        })
        .await?;
    state
        .stores
        .audit
        .append_audit(
            &user.id.to_string(),
            AuditAction::DocumentCreate,
            &doc.id.to_string(),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(doc.into())))
}

pub(crate) async fn list(
    State(state): State<ApiState>,
    Path(ws): Path<WorkspaceId>,
    Extension(CurrentUser(user)): Extension<CurrentUser>,
) -> Result<Json<Vec<DocumentResponse>>, ApiError> {
    state
        .stores
        .workspaces
        .get_workspace(ws)
        .await?
        .ok_or(ApiError::NotFound)?;
    // Access is document-scoped (spec §4 has no workspace membership), so
    // the listing is filtered to documents the caller is a member of.
    // TODO(ws-membership): replace the per-document role probe with a join
    // once workspace-scope membership lands.
    let mut visible = Vec::new();
    for doc in state.stores.workspaces.list_documents(ws).await? {
        if state
            .stores
            .members
            .get_member_role(doc.id, user.id)
            .await?
            .is_some()
        {
            visible.push(doc.into());
        }
    }
    Ok(Json(visible))
}

pub(crate) async fn get_meta(
    State(state): State<ApiState>,
    Path(doc): Path<DocumentId>,
    Extension(CurrentUser(user)): Extension<CurrentUser>,
) -> Result<Json<DocumentResponse>, ApiError> {
    let (meta, _role) = require_doc_role(&state, doc, user.id, Action::ReadMetadata).await?;
    Ok(Json(meta.into()))
}

pub(crate) async fn snapshot(
    State(state): State<ApiState>,
    Path(doc): Path<DocumentId>,
    Extension(CurrentUser(user)): Extension<CurrentUser>,
) -> Result<impl IntoResponse, ApiError> {
    let (meta, _role) = require_doc_role(&state, doc, user.id, Action::ReadContent).await?;
    let ptr = meta.snapshot_ptr.ok_or(ApiError::NotFound)?;
    // Ciphertext or plaintext according to tier — the server returns the
    // stored bytes either way (ADR-C013).
    let bytes = state.blob.get(&ptr).await?;
    Ok(([(header::CONTENT_TYPE, "application/octet-stream")], bytes))
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct PutSnapshotQuery {
    /// Highest oplog sequence the uploaded snapshot incorporates. The writer
    /// asserts coverage — under Tier 2 the server cannot verify ciphertext
    /// by construction, so this is part of the writer trust model.
    up_to: i64,
}

/// `PUT /v1/documents/{doc}/snapshot` — client-produced snapshot upload.
///
/// This is how Tier-2 documents get compacted (ADR-C013/C014: the server
/// compacts nothing it cannot read; clients upload encrypted snapshots and
/// the covered oplog ciphertext is dropped). Also usable on Tier 0/1 by a
/// client that compacted locally. The snapshot pointer only moves forward;
/// a stale upload gets `409` and changes nothing.
pub(crate) async fn put_snapshot(
    State(state): State<ApiState>,
    Path(doc): Path<DocumentId>,
    Query(query): Query<PutSnapshotQuery>,
    Extension(CurrentUser(user)): Extension<CurrentUser>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (meta, _role) = require_doc_role(&state, doc, user.id, Action::WriteContent).await?;
    if body.is_empty() {
        return Err(ApiError::Validation(
            "snapshot body must not be empty".into(),
        ));
    }
    if query.up_to <= meta.snapshot_seq {
        return Err(ApiError::SnapshotSuperseded);
    }
    // Same safety order as server-side compaction: durable snapshot →
    // guarded pointer advance → truncate only on winning the guard.
    let ptr = state
        .blob
        .put_snapshot(doc, query.up_to, body.to_vec())
        .await?;
    if !state
        .stores
        .documents
        .set_snapshot(doc, &ptr, query.up_to)
        .await?
    {
        state.blob.delete(&ptr).await?;
        return Err(ApiError::SnapshotSuperseded);
    }
    state.collab.oplog.truncate_up_to(doc, query.up_to).await?;
    Ok(Json(serde_json::json!({ "snapshot_seq": query.up_to })))
}
