// SPDX-License-Identifier: Apache-2.0

//! `POST /v1/documents/{doc}/members` — role grants (ADR-C017).

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use loki_model::{Action, DocumentId, EncryptionTier};
use loki_server_audit::AuditAction;
use loki_server_store::DocMemberRecord;

use crate::auth_mw::CurrentUser;
use crate::dto::{AddMemberRequest, MemberResponse};
use crate::error::ApiError;
use crate::routes::require_doc_role;
use crate::state::ApiState;

pub(crate) async fn add(
    State(state): State<ApiState>,
    Path(doc): Path<DocumentId>,
    Extension(CurrentUser(user)): Extension<CurrentUser>,
    Json(request): Json<AddMemberRequest>,
) -> Result<(StatusCode, Json<MemberResponse>), ApiError> {
    let (meta, _role) = require_doc_role(&state, doc, user.id, Action::ManageMembers).await?;
    state
        .stores
        .users
        .get_user(request.user_id)
        .await?
        .ok_or(ApiError::Validation("user does not exist".into()))?;
    // Under Tier 2 a grant is only meaningful with the client-driven DEK
    // re-wrap for the new member (ADR-C014/C017) — content access is keys,
    // not rows.
    if meta.tier == EncryptionTier::ZeroKnowledge && request.dek_wrapped_for_user.is_none() {
        return Err(ApiError::Validation(
            "dek_wrapped_for_user is required for zero-knowledge documents".into(),
        ));
    }
    let member = DocMemberRecord {
        doc_id: doc,
        user_id: request.user_id,
        role: request.role,
        dek_wrapped_for_user: request.dek_wrapped_for_user,
    };
    state.stores.members.upsert_member(&member).await?;
    state
        .stores
        .audit
        .append_audit(
            &user.id.to_string(),
            AuditAction::AclChange,
            &format!("{doc}:{}:{}", member.user_id, member.role.as_str()),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(member.into())))
}
