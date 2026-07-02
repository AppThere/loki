// SPDX-License-Identifier: Apache-2.0

//! Typed API errors rendered as `application/problem+json` (RFC 9457).

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use loki_server_auth::AccessError;
use loki_server_collab::BusError;
use loki_server_store::StoreError;
use serde::Serialize;

/// Every error a handler can surface. The `code` (problem `type`) is a
/// stable contract; messages are for developers, not end users (client UIs
/// localize by code via `loki_i18n`).
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Missing or invalid bearer token.
    #[error("authentication required")]
    Unauthorized,
    /// The caller's role does not permit the action.
    #[error("forbidden")]
    Forbidden,
    /// The resource does not exist — also returned for resources the caller
    /// is not a member of, so existence is not an oracle.
    #[error("not found")]
    NotFound,
    /// The request body failed validation.
    #[error("invalid request: {0}")]
    Validation(String),
    /// ADR-C015: server-side processing is disabled for Tier-2 documents.
    #[error("server-side processing is disabled for zero-knowledge documents")]
    E2eeCapabilityDisabled,
    /// The capability is specified but not yet implemented (returned instead
    /// of silently succeeding).
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
    /// Key wrapping/unwrapping failure.
    #[error("cryptography failure")]
    Crypto(#[from] loki_crypto::CryptoError),
    /// Persistence failure.
    #[error("storage failure")]
    Store(#[source] StoreError),
    /// Fan-out failure.
    #[error("collaboration bus failure")]
    Bus(#[from] BusError),
}

impl ApiError {
    /// Stable problem `type` code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::NotFound => "not-found",
            Self::Validation(_) => "validation-failed",
            Self::E2eeCapabilityDisabled => "e2ee-capability-disabled",
            Self::NotImplemented(_) => "not-implemented",
            Self::Crypto(_) => "crypto-error",
            Self::Store(_) => "storage-error",
            Self::Bus(_) => "bus-error",
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            // The spec pins the Tier-2 rejection to 409 (spec §3).
            Self::E2eeCapabilityDisabled => StatusCode::CONFLICT,
            Self::NotImplemented(_) => StatusCode::NOT_IMPLEMENTED,
            Self::Crypto(_) | Self::Store(_) | Self::Bus(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<StoreError> for ApiError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::NotFound => Self::NotFound,
            other => Self::Store(other),
        }
    }
}

impl From<AccessError> for ApiError {
    fn from(error: AccessError) -> Self {
        match error {
            // Non-members see 404, not 403: membership is not an oracle.
            AccessError::NotMember => Self::NotFound,
            AccessError::Forbidden { .. } => Self::Forbidden,
        }
    }
}

#[derive(Serialize)]
struct Problem {
    /// Stable error code, namespaced as a URN.
    r#type: String,
    title: String,
    status: u16,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        if status.is_server_error() {
            // Log the cause server-side; never leak internals in the body.
            tracing::error!(error = %self, "request failed");
        }
        let problem = Problem {
            r#type: format!("urn:appthere:loki:error:{}", self.code()),
            title: self.to_string(),
            status: status.as_u16(),
        };
        let body = serde_json::to_string(&problem).unwrap_or_else(|_| String::from("{}"));
        (
            status,
            [(header::CONTENT_TYPE, "application/problem+json")],
            body,
        )
            .into_response()
    }
}
