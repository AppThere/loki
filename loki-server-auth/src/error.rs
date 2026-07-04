// SPDX-License-Identifier: Apache-2.0

//! Typed authentication errors.

/// Why a bearer token was rejected.
///
/// Handlers map every variant to `401 Unauthorized`; the distinction exists
/// for logs and the audit trail (`AuditAction::AuthDenied`), never for the
/// response body — error bodies must not become a validation oracle.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// The token could not be parsed or its signature/claims failed
    /// validation (bad signature, expired, wrong issuer/audience, …).
    #[error("token rejected: {0}")]
    InvalidToken(#[from] jsonwebtoken::errors::Error),
    /// The token references a signing key this server does not know.
    #[error("no verification key for kid {kid:?}")]
    UnknownKey {
        /// The `kid` header of the rejected token, if present.
        kid: Option<String>,
    },
}
