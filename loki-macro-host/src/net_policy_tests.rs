// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the pure network policy (ADR-0015 §4.1/§4.3, 8B.3). No
//! `reqwest`, no I/O — these run under either `macro-net` config.

use std::collections::BTreeSet;

use super::{
    MAX_BODY_BYTES, MAX_REDIRECTS, RedirectNext, header_is_denied, redirect_next, sanitized_headers,
};

fn allowed(origins: &[&str]) -> BTreeSet<String> {
    origins.iter().map(|s| (*s).to_string()).collect()
}

#[test]
fn bounds_are_sane() {
    // `black_box` so the checks are real runtime assertions, not const-folded
    // (which clippy forbids), while still exercising the shared bounds.
    let hops = std::hint::black_box(MAX_REDIRECTS);
    let cap = std::hint::black_box(MAX_BODY_BYTES);
    // A few hops permit legitimate CDN/apex redirects without inviting loops.
    assert!((1..=10).contains(&hops));
    // The body cap targets small API payloads, not bulk downloads.
    assert!(cap >= 1024 * 1024);
}

#[test]
fn framing_and_ambient_credential_headers_are_denied() {
    for name in [
        "Host",
        "host",
        "Content-Length",
        "Connection",
        "Transfer-Encoding",
        "Proxy-Authorization",
        "Cookie",
        "COOKIE",
    ] {
        assert!(header_is_denied(name), "{name} should be denied");
    }
}

#[test]
fn author_credentials_and_ordinary_headers_pass() {
    for name in ["Authorization", "Accept", "X-Api-Key", "User-Agent"] {
        assert!(!header_is_denied(name), "{name} should be allowed");
    }
}

#[test]
fn sanitize_strips_only_denied_headers_preserving_the_rest() {
    let headers = vec![
        ("Authorization".to_string(), "Bearer t".to_string()),
        ("Cookie".to_string(), "sid=abc".to_string()),
        ("Accept".to_string(), "application/json".to_string()),
        ("Host".to_string(), "evil.example".to_string()),
    ];
    let kept = sanitized_headers(&headers);
    assert_eq!(
        kept,
        vec![
            ("Authorization", "Bearer t"),
            ("Accept", "application/json"),
        ]
    );
}

#[test]
fn non_redirect_status_stops() {
    let a = allowed(&["https://api.example.com"]);
    assert_eq!(
        redirect_next(
            200,
            Some("https://api.example.com/x"),
            "https://api.example.com/",
            &a
        ),
        RedirectNext::Stop
    );
}

#[test]
fn redirect_without_location_stops() {
    let a = allowed(&["https://api.example.com"]);
    assert_eq!(
        redirect_next(304, None, "https://api.example.com/", &a),
        RedirectNext::Stop
    );
}

#[test]
fn redirect_to_allowed_origin_is_followed_absolute() {
    let a = allowed(&["https://api.example.com", "https://cdn.example.com"]);
    assert_eq!(
        redirect_next(
            302,
            Some("https://cdn.example.com/asset"),
            "https://api.example.com/x",
            &a,
        ),
        RedirectNext::Follow("https://cdn.example.com/asset".to_string())
    );
}

#[test]
fn relative_redirect_resolves_against_base_same_origin() {
    let a = allowed(&["https://api.example.com"]);
    assert_eq!(
        redirect_next(
            301,
            Some("/v2/thing"),
            "https://api.example.com/v1/thing",
            &a
        ),
        RedirectNext::Follow("https://api.example.com/v2/thing".to_string())
    );
}

#[test]
fn redirect_to_ungranted_origin_is_denied() {
    let a = allowed(&["https://api.example.com"]);
    assert_eq!(
        redirect_next(
            302,
            Some("https://evil.example.net/steal"),
            "https://api.example.com/x",
            &a,
        ),
        RedirectNext::Deny
    );
}

#[test]
fn redirect_to_non_https_scheme_is_bad() {
    let a = allowed(&["https://api.example.com"]);
    assert_eq!(
        redirect_next(
            302,
            Some("http://api.example.com/x"),
            "https://api.example.com/",
            &a
        ),
        RedirectNext::Bad
    );
}

#[test]
fn redirect_downgrade_to_a_userinfo_authority_is_bad() {
    // origin_of rejects userinfo authorities (credential-smuggling shape).
    let a = allowed(&["https://api.example.com"]);
    assert_eq!(
        redirect_next(
            307,
            Some("https://user:pw@api.example.com/x"),
            "https://api.example.com/",
            &a,
        ),
        RedirectNext::Bad
    );
}
