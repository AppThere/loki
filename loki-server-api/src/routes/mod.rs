// SPDX-License-Identifier: Apache-2.0

//! Route handlers, one file per resource.

pub(crate) mod blobs;
pub(crate) mod collab;
pub(crate) mod documents;
pub(crate) mod export;
pub(crate) mod gdpr;
pub(crate) mod members;
pub(crate) mod workspaces;

use loki_model::{Action, DocumentId, Role, UserId};
use loki_server_store::DocMetaRecord;

use crate::error::ApiError;
use crate::state::ApiState;

/// Liveness probe (unauthenticated).
pub(crate) async fn health() -> &'static str {
    "ok"
}

/// Loads a document and enforces that `user` may perform `action` on it.
///
/// Non-members and missing documents are indistinguishable (`404`).
pub(crate) async fn require_doc_role(
    state: &ApiState,
    doc: DocumentId,
    user: UserId,
    action: Action,
) -> Result<(DocMetaRecord, Role), ApiError> {
    let meta = state
        .stores
        .documents
        .get_document(doc)
        .await?
        .ok_or(ApiError::NotFound)?;
    let role = state.stores.members.get_member_role(doc, user).await?;
    let role = loki_server_auth::require(role, action)?;
    Ok((meta, role))
}
