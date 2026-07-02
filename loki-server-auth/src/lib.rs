// SPDX-License-Identifier: Apache-2.0

//! OIDC identity verification and RBAC checks (ADR-C017).
//!
//! The server is an OIDC **relying party**: it validates bearer tokens
//! issued by an external IdP (Keycloak / Authentik / Zitadel are the
//! documented sovereign defaults) and never stores passwords. Access
//! decisions are always server-authoritative via the role matrix in
//! `loki-model`, independent of the document's encryption tier.

#![forbid(unsafe_code)]

mod claims;
mod error;
mod jwks;
mod rbac;
mod verifier;

pub use claims::{AuthContext, Claims};
pub use error::AuthError;
pub use jwks::{DEFAULT_MIN_REFRESH, HttpJwksFetcher, JwksError, JwksFetcher, JwksKeySource};
pub use rbac::{AccessError, require};
pub use verifier::{IdentityVerifier, KeySource, OidcVerifier, StaticKeys};
