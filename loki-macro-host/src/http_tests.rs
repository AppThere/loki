// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the HTTP model: origin normalization (the grant unit) and response
//! helpers.

use super::{HttpResponse, origin_of};

#[test]
fn origin_is_https_scheme_host_and_port() {
    assert_eq!(
        origin_of("https://api.example.com/path?q=1#frag").as_deref(),
        Some("https://api.example.com")
    );
    assert_eq!(
        origin_of("https://API.Example.COM:8443/x").as_deref(),
        Some("https://api.example.com:8443")
    );
    assert_eq!(
        origin_of("https://host.example.com").as_deref(),
        Some("https://host.example.com")
    );
}

#[test]
fn non_https_and_malformed_urls_have_no_origin() {
    assert_eq!(origin_of("http://example.com"), None); // plaintext
    assert_eq!(origin_of("ftp://example.com"), None);
    assert_eq!(origin_of("file:///etc/passwd"), None);
    assert_eq!(origin_of("https://"), None); // empty authority
    assert_eq!(origin_of("not a url"), None);
    // Userinfo is rejected (spoofing / ambient-credential smuggling).
    assert_eq!(origin_of("https://user:pw@evil.example.com/"), None);
    assert_eq!(origin_of("https://good.com@evil.com/"), None);
}

#[test]
fn the_default_port_normalizes_away() {
    // Regression: `origin_of` hand-parsed the authority while redirect targets
    // were resolved through `url::Url`, which drops the scheme-default port. A
    // grant taken on an explicit `:443` URL therefore did not match the origin of
    // its own same-host redirect, and the hop was refused as cross-origin.
    assert_eq!(
        origin_of("https://api.example.com:443/x").as_deref(),
        Some("https://api.example.com")
    );
    assert_eq!(
        origin_of("https://api.example.com:443/x"),
        origin_of("https://api.example.com/y"),
    );
    // A non-default port is still part of the origin.
    assert_ne!(
        origin_of("https://api.example.com:8443/x"),
        origin_of("https://api.example.com/x"),
    );
}

#[test]
fn an_idn_host_normalizes_to_punycode() {
    // Same class of bug: the client resolves IDN hosts to punycode, so the origin
    // check must too, or a grant never matches the request that follows it.
    let unicode = origin_of("https://bücher.example/x");
    assert_eq!(unicode.as_deref(), Some("https://xn--bcher-kva.example"));
    assert_eq!(unicode, origin_of("https://xn--bcher-kva.example/y"));
}

#[test]
fn response_body_and_headers() {
    let response = HttpResponse {
        status: 200,
        headers: vec![("Content-Type".to_owned(), "text/plain".to_owned())],
        body: b"hello".to_vec(),
    };
    assert_eq!(response.body_as_string(), "hello");
    // Header lookup is case-insensitive.
    assert_eq!(response.header("content-type"), Some("text/plain"));
    assert_eq!(response.header("X-Missing"), None);
}

#[test]
fn invalid_utf8_body_decodes_lossily() {
    let response = HttpResponse {
        status: 200,
        headers: Vec::new(),
        body: vec![0xff, 0xfe, b'a'],
    };
    // No panic; the replacement char stands in for the invalid bytes.
    assert!(response.body_as_string().ends_with('a'));
}
