// SPDX-License-Identifier: Apache-2.0

//! `WS /v1/documents/{doc}/collab` — the Loro relay endpoint (ADR-C013).

use axum::Extension;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path, Query, State};
use axum::response::Response;
use loki_model::{Action, DocumentId};
use loki_server_collab::drive_socket;
use serde::Deserialize;

use crate::auth_mw::CurrentUser;
use crate::error::ApiError;
use crate::routes::require_doc_role;
use crate::state::ApiState;

#[derive(Debug, Deserialize)]
pub(crate) struct CollabQuery {
    /// The last oplog sequence the client already holds (`0` after a fresh
    /// snapshot download); the server replays everything newer on connect.
    #[serde(default)]
    after: i64,
}

pub(crate) async fn upgrade(
    State(state): State<ApiState>,
    Path(doc): Path<DocumentId>,
    Query(query): Query<CollabQuery>,
    Extension(CurrentUser(user)): Extension<CurrentUser>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    // AuthN + RBAC complete *before* the upgrade (ADR-C017); the socket
    // itself only moves opaque frames.
    let (_meta, role) = require_doc_role(&state, doc, user.id, Action::ReadContent).await?;
    let can_write = role.allows(Action::WriteContent);
    let relay = state.collab.open_relay(doc, user.id, can_write);
    Ok(ws.on_upgrade(move |socket| drive_socket(socket, relay, query.after)))
}
