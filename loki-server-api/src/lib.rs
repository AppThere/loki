// SPDX-License-Identifier: Apache-2.0

//! The REST + WebSocket API surface (spec §3).
//!
//! Everything is versioned under `/v1` and JSON except the snapshot/blob
//! bodies (bytes) and the collaboration WebSocket. Errors are typed
//! ([`ApiError`]) and rendered as `application/problem+json` with stable
//! codes; `e2ee-capability-disabled` is the canonical Tier-2 rejection
//! (ADR-C015).

#![forbid(unsafe_code)]

mod auth_mw;
mod dto;
mod error;
mod routes;
mod state;

pub use auth_mw::CurrentUser;
pub use dto::{
    AddMemberRequest, BlobCreatedResponse, CreateDocumentRequest, CreateWorkspaceRequest,
    DocumentResponse, GdprExportResponse, MemberResponse, WorkspaceResponse,
};
pub use error::ApiError;
pub use state::ApiState;

use axum::Router;
use axum::middleware;
use axum::routing::{get, post};

/// Builds the `/v1` router. `state` carries every port the handlers use.
pub fn router(state: ApiState) -> Router {
    let authed = Router::new()
        .route("/v1/workspaces", post(routes::workspaces::create))
        .route(
            "/v1/workspaces/{ws}/documents",
            get(routes::documents::list).post(routes::documents::create),
        )
        .route("/v1/documents/{doc}", get(routes::documents::get_meta))
        .route(
            "/v1/documents/{doc}/snapshot",
            get(routes::documents::snapshot).put(routes::documents::put_snapshot),
        )
        .route("/v1/documents/{doc}/members", post(routes::members::add))
        .route("/v1/documents/{doc}/blobs", post(routes::blobs::upload))
        .route("/v1/documents/{doc}/collab", get(routes::collab::upgrade))
        .route("/v1/documents/{doc}/export", post(routes::export::request))
        .route("/v1/gdpr/export", get(routes::gdpr::export))
        .route("/v1/gdpr/erase", post(routes::gdpr::erase))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_mw::require_auth,
        ));

    Router::new()
        .route("/healthz", get(routes::health))
        .merge(authed)
        .with_state(state)
}
