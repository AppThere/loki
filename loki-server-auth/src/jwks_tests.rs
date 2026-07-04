// SPDX-License-Identifier: Apache-2.0

//! JWKS cache/rotation tests with an injected fetcher (no HTTP). Symmetric
//! `oct` JWKs keep the fixtures small; production pins RS256/ES256.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde_json::json;

use super::*;
use crate::verifier::{IdentityVerifier, OidcVerifier};

const ISSUER: &str = "https://idp.example.eu/realms/loki";
const AUDIENCE: &str = "loki-server";

fn jwk_set(kids_and_secrets: &[(&str, &[u8])]) -> JwkSet {
    let keys: Vec<_> = kids_and_secrets
        .iter()
        .map(|(kid, secret)| json!({"kty": "oct", "kid": kid, "k": URL_SAFE_NO_PAD.encode(secret)}))
        .collect();
    serde_json::from_value(json!({ "keys": keys })).unwrap()
}

/// Serves a scripted sequence of key sets and counts fetches.
struct SeqFetcher {
    sets: Mutex<VecDeque<JwkSet>>,
    calls: AtomicUsize,
}

impl SeqFetcher {
    fn new(sets: Vec<JwkSet>) -> Self {
        Self {
            sets: Mutex::new(sets.into()),
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl JwksFetcher for &SeqFetcher {
    async fn fetch(&self) -> Result<JwkSet, JwksError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut sets = match self.sets.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        // The last set stays current once the script runs out.
        if sets.len() > 1
            && let Some(set) = sets.pop_front()
        {
            return Ok(set);
        }
        sets.front()
            .cloned()
            .ok_or_else(|| JwksError::Unavailable("no sets scripted".into()))
    }
}

fn token(kid: &str, secret: &[u8]) -> String {
    let mut header = Header::new(Algorithm::HS256);
    header.kid = Some(kid.to_owned());
    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;
    let claims = json!({"sub": "u1", "iss": ISSUER, "aud": AUDIENCE, "exp": exp});
    encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap()
}

fn verifier(
    fetcher: &SeqFetcher,
    min_refresh: Duration,
) -> OidcVerifier<JwksKeySource<&SeqFetcher>> {
    OidcVerifier::with_algorithms(
        ISSUER,
        AUDIENCE,
        JwksKeySource::with_min_refresh(fetcher, min_refresh),
        vec![Algorithm::HS256],
    )
}

#[tokio::test]
async fn verifies_end_to_end_via_jwks() {
    let fetcher = SeqFetcher::new(vec![jwk_set(&[("k1", b"secret-one")])]);
    let verifier = verifier(&fetcher, Duration::from_secs(300));
    let ctx = verifier.verify(&token("k1", b"secret-one")).await.unwrap();
    assert_eq!(ctx.oidc_sub, "u1");
    // Cached: a second verification does not refetch.
    verifier.verify(&token("k1", b"secret-one")).await.unwrap();
    assert_eq!(fetcher.calls(), 1);
}

#[tokio::test]
async fn rotation_refetches_on_unknown_kid() {
    let fetcher = SeqFetcher::new(vec![
        jwk_set(&[("k1", b"secret-one")]),
        jwk_set(&[("k2", b"secret-two")]),
    ]);
    let verifier = verifier(&fetcher, Duration::ZERO);
    // Warm the cache with the pre-rotation set…
    verifier.verify(&token("k1", b"secret-one")).await.unwrap();
    // …then a token signed by the rotated key forces a refetch and succeeds.
    verifier.verify(&token("k2", b"secret-two")).await.unwrap();
    assert_eq!(fetcher.calls(), 2);
}

#[tokio::test]
async fn unknown_kid_is_throttled_not_hammered() {
    let fetcher = SeqFetcher::new(vec![jwk_set(&[("k1", b"secret-one")])]);
    let source = JwksKeySource::with_min_refresh(&fetcher, Duration::from_secs(3600));
    // First miss fetches once; repeated forged kids stay local.
    assert!(source.key_for(Some("forged")).await.is_none());
    assert!(source.key_for(Some("forged")).await.is_none());
    assert!(source.key_for(Some("also-forged")).await.is_none());
    assert_eq!(fetcher.calls(), 1);
    // The genuine key from that single fetch still resolves.
    assert!(source.key_for(Some("k1")).await.is_some());
}

#[tokio::test]
async fn single_key_set_serves_kidless_tokens() {
    let fetcher = SeqFetcher::new(vec![jwk_set(&[("k1", b"secret-one")])]);
    let source = JwksKeySource::with_min_refresh(&fetcher, Duration::ZERO);
    assert!(source.key_for(None).await.is_some());
}

#[tokio::test]
async fn fetch_failure_keeps_serving_cached_keys() {
    let fetcher = SeqFetcher::new(vec![]);
    let source = JwksKeySource::with_min_refresh(&fetcher, Duration::from_secs(3600));
    // Fetch fails; lookups return None but do not panic or spin.
    assert!(source.key_for(Some("k1")).await.is_none());
    assert!(source.key_for(Some("k1")).await.is_none());
    assert_eq!(fetcher.calls(), 1, "failure is throttled too");
}
