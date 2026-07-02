// SPDX-License-Identifier: Apache-2.0

//! GDPR data-subject operations (ADR-C020).

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Extension, Json};
use loki_server_audit::AuditAction;

use crate::auth_mw::CurrentUser;
use crate::dto::GdprExportResponse;
use crate::error::ApiError;
use crate::state::ApiState;

/// `GET /v1/gdpr/export` — data portability for the calling user.
pub(crate) async fn export(
    State(state): State<ApiState>,
    Extension(CurrentUser(user)): Extension<CurrentUser>,
) -> Result<Json<GdprExportResponse>, ApiError> {
    state
        .stores
        .audit
        .append_audit(
            &user.id.to_string(),
            AuditAction::GdprExport,
            &user.id.to_string(),
        )
        .await?;
    Ok(Json(user.into()))
}

/// `POST /v1/gdpr/erase` — right to erasure for the calling user's account
/// data. Personal fields are stripped; the account row remains so document
/// history and the audit chain stay intact. Per-document content erasure is
/// the document owner's crypto-shredding operation, not this endpoint.
pub(crate) async fn erase(
    State(state): State<ApiState>,
    Extension(CurrentUser(user)): Extension<CurrentUser>,
) -> Result<StatusCode, ApiError> {
    state.stores.users.anonymize_user(user.id).await?;
    state
        .stores
        .audit
        .append_audit(
            &user.id.to_string(),
            AuditAction::GdprErase,
            &user.id.to_string(),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
