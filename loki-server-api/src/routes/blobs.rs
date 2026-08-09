// SPDX-License-Identifier: Apache-2.0

//! `POST/GET /v1/documents/{doc}/blobs` — attachment upload and download, with
//! app-layer at-rest encryption for Tier 0/1 (ADR-C014/C016).

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use loki_model::{Action, DocumentId};
use loki_server_store::BlobStore;
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
    let (meta, _role) = require_doc_role(&state, doc, user.id, Action::WriteContent).await?;
    if body.is_empty() {
        return Err(ApiError::Validation(
            "attachment body must not be empty".into(),
        ));
    }
    let blob_id = Uuid::new_v4().to_string();
    let key = BlobStore::attachment_key(doc, &blob_id);
    // At rest (ADR-C016): Tier 0/1 documents hold a per-document DEK (wrapped
    // under the tier KEK), so the server seals the attachment before storing it,
    // binding the ciphertext to its object key via AAD. Tier 2 has no server-side
    // DEK — the client already uploaded ciphertext, stored as received.
    let stored = seal_for_rest(&state, &meta.dek_wrapped, &key, &body)?;
    let key = state.blob.put_attachment(doc, &blob_id, stored).await?;
    Ok((StatusCode::CREATED, Json(BlobCreatedResponse { key })))
}

pub(crate) async fn download(
    State(state): State<ApiState>,
    Path((doc, blob_id)): Path<(DocumentId, String)>,
    Extension(CurrentUser(user)): Extension<CurrentUser>,
) -> Result<Bytes, ApiError> {
    let (meta, _role) = require_doc_role(&state, doc, user.id, Action::ReadContent).await?;
    let key = BlobStore::attachment_key(doc, &blob_id);
    let stored = state.blob.get(&key).await?;
    // Inverse of `seal_for_rest`: Tier 0/1 is unsealed with the per-document DEK;
    // Tier 2 is served as received (client-decrypted).
    let bytes = match &meta.dek_wrapped {
        Some(wrapped) => {
            let dek = state.tier_kek.unwrap_dek(wrapped)?;
            dek.open(&stored, key.as_bytes())?
        }
        None => stored,
    };
    Ok(Bytes::from(bytes))
}

/// Seals `plaintext` with the document's DEK (AAD = object `key`) when the
/// document carries a wrapped DEK (Tier 0/1); returns it unchanged for Tier 2.
fn seal_for_rest(
    state: &ApiState,
    dek_wrapped: &Option<loki_crypto::WrappedDek>,
    key: &str,
    plaintext: &[u8],
) -> Result<Vec<u8>, ApiError> {
    match dek_wrapped {
        Some(wrapped) => {
            let dek = state.tier_kek.unwrap_dek(wrapped)?;
            Ok(dek.seal(plaintext, key.as_bytes())?)
        }
        None => Ok(plaintext.to_vec()),
    }
}
