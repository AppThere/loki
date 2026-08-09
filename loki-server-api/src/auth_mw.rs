// SPDX-License-Identifier: Apache-2.0

//! Bearer-token middleware: verifies the token, provisions the user
//! just-in-time, and attaches the resolved account to the request.

use axum::extract::{Request, State};
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;
use loki_server_audit::AuditAction;
use loki_server_store::UserRecord;

use crate::error::ApiError;
use crate::state::ApiState;

/// The authenticated caller, resolved to a `UserRecord` row.
#[derive(Debug, Clone)]
pub struct CurrentUser(pub UserRecord);

/// Rejects the request unless it carries a valid `Authorization: Bearer`
/// token for the configured IdP (ADR-C017).
pub async fn require_auth(
    State(state): State<ApiState>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(ApiError::Unauthorized)?;
    let identity = state.verifier.verify(token).await.map_err(|error| {
        tracing::debug!(%error, "bearer token rejected");
        ApiError::Unauthorized
    })?;
    let (user, newly_provisioned) = state
        .stores
        .users
        .upsert_user_by_oidc(&identity.oidc_sub, &identity.display_name)
        .await?;
    // Audit a first login once per account (ADR-C020 auth events), keyed by the
    // provisioning boundary so we don't append on every authenticated request.
    // Best-effort: an audit-write failure must not block authentication.
    if newly_provisioned {
        let actor = user.id.to_string();
        if let Err(error) = state
            .stores
            .audit
            .append_audit(&actor, AuditAction::AuthLogin, &actor)
            .await
        {
            tracing::warn!(%error, "failed to audit first login");
        }
    }
    request.extensions_mut().insert(CurrentUser(user));
    Ok(next.run(request).await)
}
