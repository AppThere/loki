// SPDX-License-Identifier: Apache-2.0

//! Bearer-token verification against the configured OIDC issuer.

use std::collections::HashMap;

use async_trait::async_trait;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};

use crate::claims::{AuthContext, Claims};
use crate::error::AuthError;

/// Resolves the verification key for a token's `kid` header.
///
/// [`StaticKeys`] serves keys loaded at startup. A JWKS-over-HTTP source
/// (with rotation) is deliberately deferred:
// TODO(oidc-jwks): fetch and cache the IdP's JWKS document (RFC 7517) with
// rotation on unknown-kid, instead of requiring keys in the config.
pub trait KeySource: Send + Sync {
    /// Returns the key for `kid`, or the default key when `kid` is `None`.
    fn key_for(&self, kid: Option<&str>) -> Option<&DecodingKey>;
}

/// A fixed key set loaded from configuration.
pub struct StaticKeys {
    keys: HashMap<String, DecodingKey>,
    default_key: Option<DecodingKey>,
}

impl StaticKeys {
    /// Builds a key set; `default_key` serves tokens without a `kid` header.
    #[must_use]
    pub fn new(keys: HashMap<String, DecodingKey>, default_key: Option<DecodingKey>) -> Self {
        Self { keys, default_key }
    }

    /// A single-key set (small IdP deployments).
    #[must_use]
    pub fn single(key: DecodingKey) -> Self {
        Self {
            keys: HashMap::new(),
            default_key: Some(key),
        }
    }
}

impl KeySource for StaticKeys {
    fn key_for(&self, kid: Option<&str>) -> Option<&DecodingKey> {
        match kid {
            Some(kid) => self.keys.get(kid).or(self.default_key.as_ref()),
            None => self.default_key.as_ref(),
        }
    }
}

/// Verifies a bearer token and yields the caller's identity.
#[async_trait]
pub trait IdentityVerifier: Send + Sync {
    /// Validates `token` (the value after `Bearer `) and returns the
    /// verified identity.
    async fn verify(&self, token: &str) -> Result<AuthContext, AuthError>;
}

/// JWT verification pinned to one issuer + audience (ADR-C017).
pub struct OidcVerifier<K: KeySource> {
    issuer: String,
    audience: String,
    keys: K,
    algorithms: Vec<Algorithm>,
}

impl<K: KeySource> OidcVerifier<K> {
    /// Production constructor: asymmetric signatures only (RS256/ES256) —
    /// an IdP-shared symmetric secret would let any relying party mint
    /// tokens.
    #[must_use]
    pub fn new(issuer: impl Into<String>, audience: impl Into<String>, keys: K) -> Self {
        Self::with_algorithms(
            issuer,
            audience,
            keys,
            vec![Algorithm::RS256, Algorithm::ES256],
        )
    }

    /// Constructor with an explicit algorithm allow-list (tests use HS256).
    #[must_use]
    pub fn with_algorithms(
        issuer: impl Into<String>,
        audience: impl Into<String>,
        keys: K,
        algorithms: Vec<Algorithm>,
    ) -> Self {
        Self {
            issuer: issuer.into(),
            audience: audience.into(),
            keys,
            algorithms,
        }
    }
}

#[async_trait]
impl<K: KeySource> IdentityVerifier for OidcVerifier<K> {
    async fn verify(&self, token: &str) -> Result<AuthContext, AuthError> {
        let header = decode_header(token)?;
        let key = self
            .keys
            .key_for(header.kid.as_deref())
            .ok_or(AuthError::UnknownKey { kid: header.kid })?;
        let mut validation = Validation::default();
        validation.algorithms = self.algorithms.clone();
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);
        let data = decode::<Claims>(token, key, &validation)?;
        Ok(AuthContext::from(data.claims))
    }
}

#[cfg(test)]
#[path = "verifier_tests.rs"]
mod tests;
