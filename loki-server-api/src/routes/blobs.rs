// SPDX-License-Identifier: Apache-2.0

//! `POST /v1/documents/{doc}/blobs` — attachment/image upload.

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use loki_model::{Action, DocumentId};
use uuid::Uuid;

use crate::auth_mw::CurrentUser;
use crate::dto::BlobCreatedResponse;
use crate::error::ApiError;
use crate::routes::require_doc_role;
use crate::state::ApiState;

pub(crate) async fn upload(
    State(state): State<ApiState>,
    Path(doc): Path<DocumentId>,
    Extension(CurrentUser(user)): Extension<CurrentUser>,
    body: Bytes,
) -> Result<(StatusCode, Json<BlobCreatedResponse>), ApiError> {
    let (_meta, _role) = require_doc_role(&state, doc, user.id, Action::WriteContent).await?;
    if body.is_empty() {
        return Err(ApiError::Validation(
            "attachment body must not be empty".into(),
        ));
    }
    // Bytes are opaque: under Tier 2 the client uploads ciphertext
    // (ADR-C014); the server stores what it receives either way.
    let key = state
        .blob
        .put_attachment(doc, &Uuid::new_v4().to_string(), body.to_vec())
        .await?;
    Ok((StatusCode::CREATED, Json(BlobCreatedResponse { key })))
}
