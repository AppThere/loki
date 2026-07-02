// SPDX-License-Identifier: Apache-2.0

//! `POST /v1/workspaces`

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Extension, Json};
use chrono::Utc;
use loki_model::WorkspaceId;
use loki_server_audit::AuditAction;
use loki_server_store::WorkspaceRecord;

use crate::auth_mw::CurrentUser;
use crate::dto::{CreateWorkspaceRequest, WorkspaceResponse};
use crate::error::ApiError;
use crate::state::ApiState;

pub(crate) async fn create(
    State(state): State<ApiState>,
    Extension(CurrentUser(user)): Extension<CurrentUser>,
    Json(request): Json<CreateWorkspaceRequest>,
) -> Result<(StatusCode, Json<WorkspaceResponse>), ApiError> {
    if request.name.trim().is_empty() {
        return Err(ApiError::Validation(
            "workspace name must not be empty".into(),
        ));
    }
    let workspace = WorkspaceRecord {
        id: WorkspaceId::new(),
        name: request.name,
        // Ratified decision §6.1: the deployment default applies unless the
        // request overrides it.
        default_tier: request.default_tier.unwrap_or(state.default_tier),
        residency: state.residency.clone(),
        created_at: Utc::now(),
    };
    state.stores.workspaces.create_workspace(&workspace).await?;
    state
        .stores
        .audit
        .append_audit(
            &user.id.to_string(),
            AuditAction::WorkspaceCreate,
            &workspace.id.to_string(),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(workspace.into())))
}
