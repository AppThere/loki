// SPDX-License-Identifier: Apache-2.0

//! `POST /v1/documents/{doc}/export` — server-side export (gated, ADR-C015).

use axum::Extension;
use axum::extract::{Path, State};
use loki_model::{Action, DocumentId};

use crate::auth_mw::CurrentUser;
use crate::error::ApiError;
use crate::routes::require_doc_role;
use crate::state::ApiState;

pub(crate) async fn request(
    State(state): State<ApiState>,
    Path(doc): Path<DocumentId>,
    Extension(CurrentUser(user)): Extension<CurrentUser>,
) -> Result<(), ApiError> {
    let (meta, _role) = require_doc_role(&state, doc, user.id, Action::ReadContent).await?;
    // ADR-C015: the canonical Tier-2 rejection — enforced before anything
    // else so the exclusivity is visible, not a downstream render failure.
    if !meta.tier.allows_server_side_processing() {
        return Err(ApiError::E2eeCapabilityDisabled);
    }
    // TODO(headless-c021): enqueue an apalis export job for the headless
    // render/print/convert worker (LOKI_HEADLESS_SERVER_SPEC.md). Until the
    // worker exists this is an honest 501, not a silent success.
    Err(ApiError::NotImplemented(
        "export jobs require the headless worker (ADR-C021)",
    ))
}
