// SPDX-License-Identifier: Apache-2.0

//! Verifier tests (HS256 keys — production pins RS256/ES256).

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
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

/// Signs `claims` with an explicit header and secret, so a test can vary the
/// algorithm, the `kid`, or the signing key one at a time.
fn signed(header: Header, claims: serde_json::Value, secret: &[u8]) -> String {
    encode(&header, &claims, &EncodingKey::from_secret(secret)).unwrap()
}

fn kid_header(kid: &str) -> Header {
    let mut header = Header::new(Algorithm::HS256);
    header.kid = Some(kid.to_owned());
    header
}

/// Claims that pass every non-signature check, so a rejection can only come
/// from the signature, the algorithm, or the key lookup.
fn valid_claims() -> serde_json::Value {
    json!({"sub": "u", "iss": ISSUER, "aud": AUDIENCE, "exp": future_exp()})
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

#[tokio::test]
async fn token_signed_with_a_foreign_key_is_rejected() {
    // Every claim is valid and the header is the ordinary HS256 one, so the
    // *only* difference from an accepted token is the signing key: this
    // rejection can come from nothing but signature validation.
    let forged = signed(Header::default(), valid_claims(), b"attacker-secret");
    assert!(matches!(
        verifier().verify(&forged).await,
        Err(AuthError::InvalidToken(_))
    ));
    // Control: the same claims signed with the real key are accepted, so the
    // rejection above cannot be attributed to the claims themselves.
    assert!(verifier().verify(&token(valid_claims())).await.is_ok());
}

#[tokio::test]
async fn algorithm_outside_the_allow_list_is_rejected() {
    // Signed with the *correct* secret — only `alg` falls outside the
    // HS256-only allow-list, which is the guard that keeps the production
    // RS256/ES256 pin meaningful.
    let hs384 = signed(Header::new(Algorithm::HS384), valid_claims(), SECRET);
    assert!(matches!(
        verifier().verify(&hs384).await,
        Err(AuthError::InvalidToken(_))
    ));
    // Control: widening the allow-list to include HS384 accepts that very
    // same token, so the rejection came from the allow-list and not from the
    // token being unverifiable for some other reason.
    let widened = OidcVerifier::with_algorithms(
        ISSUER,
        AUDIENCE,
        StaticKeys::single(DecodingKey::from_secret(SECRET)),
        vec![Algorithm::HS256, Algorithm::HS384],
    );
    assert!(widened.verify(&hs384).await.is_ok());
}

#[tokio::test]
async fn unsigned_alg_none_token_is_rejected() {
    // `alg: none` never reaches key lookup: the header does not deserialize
    // into jsonwebtoken's `Algorithm`. Hand-rolled because no encoder will
    // emit it.
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none","typ":"JWT"}"#);
    let payload = URL_SAFE_NO_PAD.encode(valid_claims().to_string().as_bytes());
    let unsigned = format!("{header}.{payload}.");
    assert!(matches!(
        verifier().verify(&unsigned).await,
        Err(AuthError::InvalidToken(_))
    ));
}

#[tokio::test]
async fn default_key_serves_an_unrecognised_kid_but_never_a_foreign_signature() {
    // Pins the `StaticKeys` fallback (verifier.rs `key_for`): the single-key
    // set built by `StaticKeys::single` has an empty `kid` map, so *every*
    // kid — including one the attacker chose — resolves to the default key.
    // That is deliberate: `single` is the static-PEM deployment mode, where
    // the operator installed exactly one trusted key and real IdPs still
    // stamp a `kid` on their tokens. It is not a bypass, because the
    // signature is checked against that operator-configured key regardless.
    let by_fallback = signed(kid_header("forged-kid"), valid_claims(), SECRET);
    assert!(
        verifier().verify(&by_fallback).await.is_ok(),
        "an unrecognised kid falls back to the default key"
    );

    // The inversion that makes the fallback safe: a forged kid does not
    // launder a foreign signature.
    let foreign = signed(kid_header("forged-kid"), valid_claims(), b"attacker-secret");
    assert!(matches!(
        verifier().verify(&foreign).await,
        Err(AuthError::InvalidToken(_))
    ));
}

#[tokio::test]
async fn a_mapped_kid_uses_its_own_key_not_the_default() {
    // With a populated `kid` map the fallback must not shadow it: a token
    // labelled `k1` verifies against k1's key and against nothing else.
    let mut keys = HashMap::new();
    keys.insert(String::from("k1"), DecodingKey::from_secret(b"secret-one"));
    let mapped = OidcVerifier::with_algorithms(
        ISSUER,
        AUDIENCE,
        StaticKeys::new(keys, Some(DecodingKey::from_secret(SECRET))),
        vec![Algorithm::HS256],
    );

    assert!(
        mapped
            .verify(&signed(kid_header("k1"), valid_claims(), b"secret-one"))
            .await
            .is_ok()
    );
    // Signed with the *default* key but labelled `k1` — accepting this would
    // mean the map lookup was skipped in favour of the default.
    assert!(matches!(
        mapped
            .verify(&signed(kid_header("k1"), valid_claims(), SECRET))
            .await,
        Err(AuthError::InvalidToken(_))
    ));
}
