// SPDX-License-Identifier: Apache-2.0

//! Token claims and the verified identity handed to request handlers.

use serde::{Deserialize, Serialize};

/// The OIDC claims the server consumes. Unknown claims are ignored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Stable subject identifier (scoped to the issuer).
    pub sub: String,
    /// Issuer URL; must match the configured IdP.
    pub iss: String,
    /// Expiry (validated by the JWT library).
    pub exp: u64,
    /// Human display name, when the IdP provides one.
    #[serde(default)]
    pub name: Option<String>,
    /// Fallback display identifier.
    #[serde(default)]
    pub preferred_username: Option<String>,
}

/// A verified identity, attached to the request after token validation.
///
/// This is IdP-asserted identity only; the `UserId` row is resolved (and
/// just-in-time provisioned) by the API layer via `UserStore`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthContext {
    /// OIDC subject.
    pub oidc_sub: String,
    /// Display name (best available claim; falls back to the subject).
    pub display_name: String,
}

impl From<Claims> for AuthContext {
    fn from(claims: Claims) -> Self {
        let display_name = claims
            .name
            .or(claims.preferred_username)
            .unwrap_or_else(|| claims.sub.clone());
        Self {
            oidc_sub: claims.sub,
            display_name,
        }
    }
}
