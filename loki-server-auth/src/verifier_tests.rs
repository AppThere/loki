// SPDX-License-Identifier: Apache-2.0

//! Verifier tests (HS256 keys — production pins RS256/ES256).

use jsonwebtoken::{EncodingKey, Header, encode};
use serde_json::json;

use super::*;

const SECRET: &[u8] = b"test-secret";
const ISSUER: &str = "https://idp.example.eu/realms/loki";
const AUDIENCE: &str = "loki-server";

fn verifier() -> OidcVerifier<StaticKeys> {
    OidcVerifier::with_algorithms(
        ISSUER,
        AUDIENCE,
        StaticKeys::single(DecodingKey::from_secret(SECRET)),
        vec![Algorithm::HS256],
    )
}

fn token(claims: serde_json::Value) -> String {
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(SECRET),
    )
    .unwrap()
}

fn future_exp() -> u64 {
    (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs())
        + 3600
}

#[tokio::test]
async fn valid_token_yields_identity() {
    let claims = json!({
        "sub": "user-123", "iss": ISSUER, "aud": AUDIENCE,
        "exp": future_exp(), "name": "Ada Lovelace",
    });
    let ctx = verifier().verify(&token(claims)).await.unwrap();
    assert_eq!(ctx.oidc_sub, "user-123");
    assert_eq!(ctx.display_name, "Ada Lovelace");
}

#[tokio::test]
async fn display_name_falls_back_to_sub() {
    let claims = json!({
        "sub": "user-9", "iss": ISSUER, "aud": AUDIENCE, "exp": future_exp(),
    });
    let ctx = verifier().verify(&token(claims)).await.unwrap();
    assert_eq!(ctx.display_name, "user-9");
}

#[tokio::test]
async fn wrong_issuer_audience_or_expiry_is_rejected() {
    for claims in [
        json!({"sub": "u", "iss": "https://evil.example", "aud": AUDIENCE, "exp": future_exp()}),
        json!({"sub": "u", "iss": ISSUER, "aud": "other-api", "exp": future_exp()}),
        json!({"sub": "u", "iss": ISSUER, "aud": AUDIENCE, "exp": 1_000_000}),
    ] {
        assert!(matches!(
            verifier().verify(&token(claims)).await,
            Err(AuthError::InvalidToken(_))
        ));
    }
}

#[tokio::test]
async fn unknown_kid_is_rejected() {
    let empty = OidcVerifier::with_algorithms(
        ISSUER,
        AUDIENCE,
        StaticKeys::new(HashMap::new(), None),
        vec![Algorithm::HS256],
    );
    let claims = json!({"sub": "u", "iss": ISSUER, "aud": AUDIENCE, "exp": future_exp()});
    assert!(matches!(
        empty.verify(&token(claims)).await,
        Err(AuthError::UnknownKey { .. })
    ));
}

#[tokio::test]
async fn garbage_token_is_rejected() {
    assert!(matches!(
        verifier().verify("not-a-jwt").await,
        Err(AuthError::InvalidToken(_))
    ));
}
