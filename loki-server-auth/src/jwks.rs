// SPDX-License-Identifier: Apache-2.0

//! JWKS (RFC 7517) key source: fetches the IdP's published key set over
//! HTTPS, caches it, and refetches on an unknown `kid` — which is exactly
//! what happens when the IdP rotates its signing keys. Refetches are
//! throttled so a flood of forged-`kid` tokens cannot hammer the IdP.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use jsonwebtoken::DecodingKey;
use jsonwebtoken::jwk::JwkSet;
use tokio::sync::RwLock;

use crate::verifier::KeySource;

/// Default minimum interval between JWKS refetches.
pub const DEFAULT_MIN_REFRESH: Duration = Duration::from_secs(300);

/// Fetches the raw JWKS document. Split from the cache so tests can inject
/// rotations without an HTTP server.
#[async_trait]
pub trait JwksFetcher: Send + Sync {
    /// Retrieves the current key set from the IdP.
    async fn fetch(&self) -> Result<JwkSet, JwksError>;
}

/// JWKS retrieval failures.
#[derive(Debug, thiserror::Error)]
pub enum JwksError {
    /// The HTTP request failed or returned a non-success status.
    #[error("jwks request failed: {0}")]
    Http(#[from] reqwest::Error),
    /// The source is unavailable (used by non-HTTP fetchers).
    #[error("jwks unavailable: {0}")]
    Unavailable(String),
}

/// Fetches the JWKS from the IdP's `jwks_uri` (e.g. Keycloak's
/// `…/protocol/openid-connect/certs`).
pub struct HttpJwksFetcher {
    url: String,
    client: reqwest::Client,
}

impl HttpJwksFetcher {
    /// Creates a fetcher for the given JWKS URL.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl JwksFetcher for HttpJwksFetcher {
    async fn fetch(&self) -> Result<JwkSet, JwksError> {
        Ok(self
            .client
            .get(&self.url)
            .send()
            .await?
            .error_for_status()?
            .json::<JwkSet>()
            .await?)
    }
}

#[derive(Default)]
struct Cache {
    keys: HashMap<String, DecodingKey>,
    /// Serves `kid`-less tokens when the set holds exactly one key.
    sole_key: Option<DecodingKey>,
    last_fetch: Option<Instant>,
}

impl Cache {
    fn lookup(&self, kid: Option<&str>) -> Option<DecodingKey> {
        match kid {
            Some(kid) => self.keys.get(kid).cloned(),
            None => self.sole_key.clone(),
        }
    }

    fn rebuild(&mut self, set: &JwkSet, now: Instant) {
        self.keys.clear();
        let mut usable = Vec::new();
        for jwk in &set.keys {
            match DecodingKey::from_jwk(jwk) {
                Ok(key) => {
                    if let Some(kid) = &jwk.common.key_id {
                        self.keys.insert(kid.clone(), key.clone());
                    }
                    usable.push(key);
                }
                Err(error) => {
                    tracing::warn!(%error, "skipping unusable JWK in key set");
                }
            }
        }
        self.sole_key = match usable.as_slice() {
            [only] => Some(only.clone()),
            _ => None,
        };
        self.last_fetch = Some(now);
    }
}

/// A [`KeySource`] backed by the IdP's JWKS document (ADR-C017).
pub struct JwksKeySource<F: JwksFetcher> {
    fetcher: F,
    min_refresh: Duration,
    cache: RwLock<Cache>,
}

impl<F: JwksFetcher> JwksKeySource<F> {
    /// Creates a source with the default refresh throttle.
    #[must_use]
    pub fn new(fetcher: F) -> Self {
        Self::with_min_refresh(fetcher, DEFAULT_MIN_REFRESH)
    }

    /// Creates a source with an explicit refresh throttle.
    #[must_use]
    pub fn with_min_refresh(fetcher: F, min_refresh: Duration) -> Self {
        Self {
            fetcher,
            min_refresh,
            cache: RwLock::new(Cache::default()),
        }
    }
}

#[async_trait]
impl<F: JwksFetcher> KeySource for JwksKeySource<F> {
    async fn key_for(&self, kid: Option<&str>) -> Option<DecodingKey> {
        if let Some(key) = self.cache.read().await.lookup(kid) {
            return Some(key);
        }
        // Miss: the key set may have rotated. Take the write lock, re-check
        // (another task may have refreshed while we waited), then refetch —
        // unless a refetch ran recently, in which case the kid is genuinely
        // unknown and the token is rejected without touching the IdP.
        let mut cache = self.cache.write().await;
        if let Some(key) = cache.lookup(kid) {
            return Some(key);
        }
        let now = Instant::now();
        if let Some(last) = cache.last_fetch
            && now.duration_since(last) < self.min_refresh
        {
            return None;
        }
        match self.fetcher.fetch().await {
            Ok(set) => {
                cache.rebuild(&set, now);
                cache.lookup(kid)
            }
            Err(error) => {
                tracing::warn!(%error, "JWKS refresh failed; keeping cached keys");
                // Throttle retries even on failure so an IdP outage does not
                // turn every request into an upstream fetch.
                cache.last_fetch = Some(now);
                None
            }
        }
    }
}

#[cfg(test)]
#[path = "jwks_tests.rs"]
mod tests;
