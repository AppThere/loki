// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`NetworkPolicy`]. (`MACRO_NET_COMPILED` is a compile-time constant
//! reflecting the `macro-net` feature; the disabled-by-default behaviour is
//! exercised through the broker tests.)

use super::NetworkPolicy;

#[test]
fn disabled_is_the_default() {
    assert!(!NetworkPolicy::disabled().is_enabled());
    assert!(!NetworkPolicy::default().is_enabled());
}

#[test]
fn enabled_policy_tracks_allowed_origins() {
    let mut policy = NetworkPolicy::enabled();
    assert!(policy.is_enabled());
    assert!(!policy.allows("https://api.example.com"));

    policy.allow_origin("https://api.example.com");
    assert!(policy.allows("https://api.example.com"));
    // Distinct origins are independent — no wildcards.
    assert!(!policy.allows("https://evil.example.com"));
    assert!(!policy.allows("https://api.example.com:8443"));

    let origins: Vec<&str> = policy.origins().collect();
    assert_eq!(origins, vec!["https://api.example.com"]);
}
