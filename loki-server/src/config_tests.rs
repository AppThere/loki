// SPDX-License-Identifier: Apache-2.0

//! Configuration-validation tests (ADR-C019 sovereignty gates).
//!
//! Every case runs against [`ServerConfig::from_vars`] with an explicit
//! variable map, so nothing here touches the process environment and the
//! tests cannot race each other.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use super::*;

const KEK_BYTES: [u8; 32] = [7u8; 32];

/// A fully valid deployment: every test below is this map with exactly one
/// variable changed, so a failure is attributable to that variable alone.
fn base() -> Vars {
    [
        ("LOKI_OBJECT_STORE", String::from("memory")),
        ("LOKI_RESIDENCY", String::from("fsn1")),
        ("DATABASE_URL", String::from("postgres://localhost/loki")),
        (
            "LOKI_OIDC_ISSUER",
            String::from("https://idp.example.eu/realms/loki"),
        ),
        ("LOKI_OIDC_AUDIENCE", String::from("loki-server")),
        (
            "LOKI_OIDC_JWKS_URL",
            String::from("https://idp.example.eu/certs"),
        ),
        ("LOKI_KEK_BASE64", STANDARD.encode(KEK_BYTES)),
    ]
    .into_iter()
    .map(|(k, v)| (String::from(k), v))
    .collect()
}

fn with(name: &str, value: &str) -> Vars {
    let mut vars = base();
    vars.insert(String::from(name), String::from(value));
    vars
}

fn without(name: &str) -> Vars {
    let mut vars = base();
    vars.remove(name);
    vars
}

/// Asserts the config was rejected as invalid, naming the offending variable.
#[track_caller]
fn assert_invalid(vars: &Vars, expected_name: &str) -> String {
    match ServerConfig::from_vars(vars) {
        Err(ConfigError::Invalid { name, reason }) => {
            assert_eq!(name, expected_name, "rejected the wrong variable");
            reason
        }
        Err(other) => panic!("expected Invalid({expected_name}), got {other:?}"),
        Ok(_) => panic!("expected Invalid({expected_name}), but the config was accepted"),
    }
}

#[track_caller]
fn assert_missing(vars: &Vars, expected_name: &str) {
    match ServerConfig::from_vars(vars) {
        Err(ConfigError::Missing(name)) => assert_eq!(name, expected_name),
        Err(other) => panic!("expected Missing({expected_name}), got {other:?}"),
        Ok(_) => panic!("expected Missing({expected_name}), but the config was accepted"),
    }
}

#[track_caller]
fn accepted(vars: &Vars) -> ServerConfig {
    match ServerConfig::from_vars(vars) {
        Ok(config) => config,
        Err(error) => panic!("expected the config to be accepted, got {error:?}"),
    }
}

// ---------------------------------------------------------------- defaults

#[test]
fn the_minimal_valid_deployment_is_accepted_with_documented_defaults() {
    let config = accepted(&base());
    assert_eq!(config.bind.to_string(), "0.0.0.0:8080");
    assert_eq!(config.default_tier, EncryptionTier::TransportAtRest);
    assert_eq!(
        config.residency,
        Residency::HetznerRegion(String::from("fsn1"))
    );
    assert_eq!(config.compact_interval, Some(Duration::from_secs(300)));
    assert_eq!(config.compact_min_entries, 256);
    assert!(matches!(config.object_store, ObjectStoreConfig::Memory));
    assert!(matches!(config.oidc_keys, OidcKeyConfig::JwksUrl(_)));
    assert_eq!(config.database_url, "postgres://localhost/loki");
    assert_eq!(config.oidc_audience, "loki-server");
}

// ------------------------------------------------------- ADR-C019: the tier gate

#[test]
fn tier_2_is_rejected_as_the_deployment_default() {
    // The gate that keeps zero-knowledge per-document opt-in (ratified
    // decision §6.1): accepting this would silently disable every
    // server-side capability.
    let reason = assert_invalid(&with("LOKI_DEFAULT_TIER", "2"), "LOKI_DEFAULT_TIER");
    assert!(
        reason.contains("per-document opt-in"),
        "unexpected reason: {reason}"
    );
}

#[test]
fn tiers_0_and_1_are_accepted_as_the_deployment_default() {
    // The inversion of the gate above: the two permitted values must pass,
    // or "tier 2 is rejected" could be satisfied by rejecting everything.
    assert_eq!(
        accepted(&with("LOKI_DEFAULT_TIER", "0")).default_tier,
        EncryptionTier::TransportAtRest
    );
    assert_eq!(
        accepted(&with("LOKI_DEFAULT_TIER", "1")).default_tier,
        EncryptionTier::CustomerManagedKeys
    );
}

#[test]
fn an_unknown_default_tier_is_rejected() {
    for raw in ["3", "-1", "one", ""] {
        let reason = assert_invalid(&with("LOKI_DEFAULT_TIER", raw), "LOKI_DEFAULT_TIER");
        assert!(
            reason.contains("unknown tier"),
            "unexpected reason: {reason}"
        );
    }
}

// -------------------------------------------------- ADR-C019: the residency pin

#[test]
fn residency_outside_the_eu_allow_list_is_rejected() {
    for region in ["us-east-1", "ash", "eu-west-1", "fsn2", ""] {
        assert!(
            matches!(
                ServerConfig::from_vars(&with("LOKI_RESIDENCY", region)),
                Err(ConfigError::Residency(ResidencyError::DisallowedRegion(_)))
            ),
            "residency {region:?} should have been rejected"
        );
    }
}

#[test]
fn allowed_eu_regions_and_labelled_self_hosting_are_accepted() {
    for region in ["fsn1", "nbg1", "hel1"] {
        assert_eq!(
            accepted(&with("LOKI_RESIDENCY", region)).residency,
            Residency::HetznerRegion(String::from(region))
        );
    }
    assert_eq!(
        accepted(&with("LOKI_RESIDENCY", "self-hosted:rack-7")).residency,
        Residency::SelfHosted(String::from("rack-7"))
    );
}

#[test]
fn self_hosted_residency_without_a_label_is_rejected() {
    assert!(matches!(
        ServerConfig::from_vars(&with("LOKI_RESIDENCY", "self-hosted:")),
        Err(ConfigError::Residency(ResidencyError::EmptySelfHostedLabel))
    ));
}

#[test]
fn residency_is_required() {
    assert_missing(&without("LOKI_RESIDENCY"), "LOKI_RESIDENCY");
}

// ------------------------------------------- ADR-C017: exactly one OIDC key source

#[test]
fn setting_both_oidc_key_sources_is_rejected() {
    let vars = with("LOKI_OIDC_RSA_PEM_FILE", "/does/not/matter.pem");
    let reason = assert_invalid(&vars, "LOKI_OIDC_JWKS_URL");
    assert!(reason.contains("not both"), "unexpected reason: {reason}");
}

#[test]
fn setting_neither_oidc_key_source_is_rejected() {
    assert_missing(&without("LOKI_OIDC_JWKS_URL"), "LOKI_OIDC_JWKS_URL");
}

#[test]
fn the_static_pem_source_is_accepted_and_read_from_disk() {
    let path = std::env::temp_dir().join(format!(
        "loki-config-test-{}-pem-source.pem",
        std::process::id()
    ));
    let contents = b"-----BEGIN PUBLIC KEY-----\nnot-a-real-key\n-----END PUBLIC KEY-----\n";
    std::fs::write(&path, contents).unwrap();

    let mut vars = without("LOKI_OIDC_JWKS_URL");
    vars.insert(
        String::from("LOKI_OIDC_RSA_PEM_FILE"),
        path.display().to_string(),
    );
    let config = accepted(&vars);
    match config.oidc_keys {
        OidcKeyConfig::StaticRsaPem(pem) => assert_eq!(pem, contents),
        OidcKeyConfig::JwksUrl(_) => panic!("expected the static-PEM source"),
    }
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn an_unreadable_pem_file_is_rejected() {
    let mut vars = without("LOKI_OIDC_JWKS_URL");
    vars.insert(
        String::from("LOKI_OIDC_RSA_PEM_FILE"),
        String::from("/nonexistent/loki-test/key.pem"),
    );
    assert_invalid(&vars, "LOKI_OIDC_RSA_PEM_FILE");
}

#[test]
fn the_oidc_issuer_and_audience_are_required() {
    assert_missing(&without("LOKI_OIDC_ISSUER"), "LOKI_OIDC_ISSUER");
    assert_missing(&without("LOKI_OIDC_AUDIENCE"), "LOKI_OIDC_AUDIENCE");
}

// ------------------------------------------------------------------- the KEK

#[test]
fn a_kek_that_is_not_base64_is_rejected() {
    let reason = assert_invalid(&with("LOKI_KEK_BASE64", "not base64!!"), "LOKI_KEK_BASE64");
    assert!(
        reason.contains("not valid base64"),
        "unexpected reason: {reason}"
    );
}

#[test]
fn a_kek_of_the_wrong_length_is_rejected() {
    // Well-formed base64, wrong key size — the check that base64 validity
    // alone must not satisfy.
    for len in [0usize, 16, 31, 33, 64] {
        let vars = with("LOKI_KEK_BASE64", &STANDARD.encode(vec![0u8; len]));
        let reason = assert_invalid(&vars, "LOKI_KEK_BASE64");
        assert!(
            !reason.contains("not valid base64"),
            "{len}-byte KEK was rejected as malformed base64, not as wrong-length: {reason}"
        );
    }
}

#[test]
fn a_32_byte_kek_is_accepted_with_surrounding_whitespace() {
    // `base64_decode` trims, so a KEK pasted with a trailing newline works.
    let padded = format!("\n{}\n", STANDARD.encode(KEK_BYTES));
    accepted(&with("LOKI_KEK_BASE64", &padded));
}

#[test]
fn the_kek_is_required() {
    assert_missing(&without("LOKI_KEK_BASE64"), "LOKI_KEK_BASE64");
}

// ------------------------------------------------------------ object storage

#[test]
fn the_object_store_accepts_memory_and_a_named_s3_bucket() {
    assert!(matches!(
        accepted(&with("LOKI_OBJECT_STORE", "memory")).object_store,
        ObjectStoreConfig::Memory
    ));
    match accepted(&with("LOKI_OBJECT_STORE", "s3://loki-docs")).object_store {
        ObjectStoreConfig::S3 { bucket } => assert_eq!(bucket, "loki-docs"),
        ObjectStoreConfig::Memory => panic!("expected the S3 backend"),
    }
}

#[test]
fn a_malformed_object_store_url_is_rejected() {
    // Note `s3://` with no bucket: the empty-bucket arm, which a naive
    // `strip_prefix` check would accept.
    for raw in ["s3://", "file:///var/loki", "memory ", "", "s3:/loki"] {
        assert_invalid(&with("LOKI_OBJECT_STORE", raw), "LOKI_OBJECT_STORE");
    }
}

#[test]
fn the_object_store_is_required() {
    assert_missing(&without("LOKI_OBJECT_STORE"), "LOKI_OBJECT_STORE");
}

// --------------------------------------------------- ADR-C013: the compactor

#[test]
fn a_zero_compact_interval_disables_the_compactor() {
    assert_eq!(
        accepted(&with("LOKI_COMPACT_INTERVAL_SECS", "0")).compact_interval,
        None
    );
    // …and a non-zero one does not, so `None` really means "disabled" and
    // not "unset".
    assert_eq!(
        accepted(&with("LOKI_COMPACT_INTERVAL_SECS", "60")).compact_interval,
        Some(Duration::from_secs(60))
    );
}

#[test]
fn a_non_numeric_compact_interval_is_rejected() {
    for raw in ["-1", "5m", "", "300.0"] {
        assert_invalid(
            &with("LOKI_COMPACT_INTERVAL_SECS", raw),
            "LOKI_COMPACT_INTERVAL_SECS",
        );
    }
}

#[test]
fn a_compact_min_entries_below_one_is_rejected() {
    for raw in ["0", "-1", "-1000"] {
        let vars = with("LOKI_COMPACT_MIN_ENTRIES", raw);
        let reason = assert_invalid(&vars, "LOKI_COMPACT_MIN_ENTRIES");
        assert!(
            reason.contains("must be >= 1"),
            "unexpected reason: {reason}"
        );
    }
    // The floor is inclusive: 1 is the smallest accepted backlog.
    assert_eq!(
        accepted(&with("LOKI_COMPACT_MIN_ENTRIES", "1")).compact_min_entries,
        1
    );
}

#[test]
fn a_non_numeric_compact_min_entries_is_rejected() {
    for raw in ["many", "", "1.5"] {
        let vars = with("LOKI_COMPACT_MIN_ENTRIES", raw);
        let reason = assert_invalid(&vars, "LOKI_COMPACT_MIN_ENTRIES");
        assert!(
            !reason.contains("must be >= 1"),
            "{raw:?} was rejected by the floor rather than as unparseable: {reason}"
        );
    }
}

// ------------------------------------------------------------- bind & database

#[test]
fn a_malformed_bind_address_is_rejected() {
    for raw in ["not-an-address", "0.0.0.0", "0.0.0.0:99999", ""] {
        assert_invalid(&with("LOKI_BIND", raw), "LOKI_BIND");
    }
}

#[test]
fn an_explicit_bind_address_overrides_the_default() {
    assert_eq!(
        accepted(&with("LOKI_BIND", "127.0.0.1:9000"))
            .bind
            .to_string(),
        "127.0.0.1:9000"
    );
}

#[test]
fn the_database_url_is_required() {
    assert_missing(&without("DATABASE_URL"), "DATABASE_URL");
}
