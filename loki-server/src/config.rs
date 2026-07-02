// SPDX-License-Identifier: Apache-2.0

//! Environment-driven configuration (ADR-C018: env + optional file; no
//! phone-home, telemetry off by default).
//!
//! Sovereignty checks happen here, at validation time (ADR-C019): the
//! residency value must be an allowed EU region or an explicit
//! `self-hosted:<label>`, and the deployment default tier can never be
//! Tier 2 (zero-knowledge is per-document opt-in, ratified decision §6.1).

use std::net::SocketAddr;
use std::time::Duration;

use loki_crypto::Kek;
use loki_model::{EncryptionTier, Residency, ResidencyError};

/// Fully validated server configuration.
pub struct ServerConfig {
    /// Listen address (`LOKI_BIND`, default `0.0.0.0:8080`).
    pub bind: SocketAddr,
    /// Postgres connection string (`DATABASE_URL`).
    pub database_url: String,
    /// Object storage: `memory` (dev/eval only) or `s3://<bucket>`
    /// (`LOKI_OBJECT_STORE`; S3 credentials/endpoint come from the standard
    /// `AWS_*` environment variables — Hetzner Object Storage and MinIO are
    /// both S3-compatible, ADR-C016).
    pub object_store: ObjectStoreConfig,
    /// Data residency recorded on new workspaces (`LOKI_RESIDENCY`).
    pub residency: Residency,
    /// Deployment default tier (`LOKI_DEFAULT_TIER`: `0` or `1`).
    pub default_tier: EncryptionTier,
    /// OIDC issuer URL (`LOKI_OIDC_ISSUER`).
    pub oidc_issuer: String,
    /// Expected token audience (`LOKI_OIDC_AUDIENCE`).
    pub oidc_audience: String,
    /// Where token-verification keys come from: `LOKI_OIDC_JWKS_URL` (the
    /// IdP's `jwks_uri`; cached, rotation-aware — the recommended setup) or
    /// `LOKI_OIDC_RSA_PEM_FILE` (a static PEM public key, for IdPs without
    /// a reachable JWKS endpoint). Exactly one must be set.
    pub oidc_keys: OidcKeyConfig,
    /// Tier-0/1 key-encryption key (`LOKI_KEK_BASE64`, 32 bytes base64).
    // TODO(kms): source the KEK from Vault/KMS/PKCS#11 instead of the
    // environment; env is acceptable for single-node self-host only.
    pub kek: Kek,
    /// Snapshot-compaction cadence (`LOKI_COMPACT_INTERVAL_SECS`, default
    /// 300; `0` disables the background compactor — ADR-C013).
    pub compact_interval: Option<Duration>,
    /// Minimum oplog backlog before a document is compacted
    /// (`LOKI_COMPACT_MIN_ENTRIES`, default 256).
    pub compact_min_entries: i64,
}

/// Source of OIDC token-verification keys (ADR-C017).
pub enum OidcKeyConfig {
    /// Fetch and cache the IdP's JWKS document, refreshing on rotation.
    JwksUrl(String),
    /// A fixed PEM-encoded RSA public key loaded at startup.
    StaticRsaPem(Vec<u8>),
}

/// Where snapshots/attachments live.
pub enum ObjectStoreConfig {
    /// In-process memory — evaluation only, nothing is durable.
    Memory,
    /// An S3-compatible bucket (Hetzner Object Storage, MinIO, …).
    S3 {
        /// Bucket name.
        bucket: String,
    },
}

/// Configuration failures (all fatal at startup).
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// A required variable is missing.
    #[error("missing required environment variable {0}")]
    Missing(&'static str),
    /// A variable could not be parsed.
    #[error("invalid value for {name}: {reason}")]
    Invalid {
        /// Variable name.
        name: &'static str,
        /// Why it was rejected.
        reason: String,
    },
    /// The residency value violates the EU pin (ADR-C019).
    #[error("residency rejected: {0}")]
    Residency(#[from] ResidencyError),
}

fn required(name: &'static str) -> Result<String, ConfigError> {
    std::env::var(name).map_err(|_| ConfigError::Missing(name))
}

fn invalid(name: &'static str, reason: impl ToString) -> ConfigError {
    ConfigError::Invalid {
        name,
        reason: reason.to_string(),
    }
}

impl ServerConfig {
    /// Reads and validates the configuration from the environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        let bind = std::env::var("LOKI_BIND")
            .unwrap_or_else(|_| String::from("0.0.0.0:8080"))
            .parse()
            .map_err(|e| invalid("LOKI_BIND", e))?;

        let object_store = match required("LOKI_OBJECT_STORE")?.as_str() {
            "memory" => ObjectStoreConfig::Memory,
            url => match url.strip_prefix("s3://") {
                Some(bucket) if !bucket.is_empty() => ObjectStoreConfig::S3 {
                    bucket: bucket.to_owned(),
                },
                _ => {
                    return Err(invalid(
                        "LOKI_OBJECT_STORE",
                        "expected `memory` or `s3://<bucket>`",
                    ));
                }
            },
        };

        let residency = Residency::parse(&required("LOKI_RESIDENCY")?)?;

        let default_tier = match std::env::var("LOKI_DEFAULT_TIER").as_deref() {
            Err(_) | Ok("0") => EncryptionTier::TransportAtRest,
            Ok("1") => EncryptionTier::CustomerManagedKeys,
            Ok("2") => {
                return Err(invalid(
                    "LOKI_DEFAULT_TIER",
                    "Tier 2 is per-document opt-in and cannot be the deployment default",
                ));
            }
            Ok(other) => {
                return Err(invalid(
                    "LOKI_DEFAULT_TIER",
                    format!("unknown tier {other}"),
                ));
            }
        };

        let oidc_keys = match (
            std::env::var("LOKI_OIDC_JWKS_URL").ok(),
            std::env::var("LOKI_OIDC_RSA_PEM_FILE").ok(),
        ) {
            (Some(url), None) => OidcKeyConfig::JwksUrl(url),
            (None, Some(pem_path)) => OidcKeyConfig::StaticRsaPem(
                std::fs::read(&pem_path).map_err(|e| invalid("LOKI_OIDC_RSA_PEM_FILE", e))?,
            ),
            (Some(_), Some(_)) => {
                return Err(invalid(
                    "LOKI_OIDC_JWKS_URL",
                    "set either LOKI_OIDC_JWKS_URL or LOKI_OIDC_RSA_PEM_FILE, not both",
                ));
            }
            (None, None) => return Err(ConfigError::Missing("LOKI_OIDC_JWKS_URL")),
        };

        let compact_interval = match std::env::var("LOKI_COMPACT_INTERVAL_SECS").as_deref() {
            Err(_) => Some(Duration::from_secs(300)),
            Ok(raw) => match raw.parse::<u64>() {
                Ok(0) => None,
                Ok(secs) => Some(Duration::from_secs(secs)),
                Err(e) => return Err(invalid("LOKI_COMPACT_INTERVAL_SECS", e)),
            },
        };
        let compact_min_entries = match std::env::var("LOKI_COMPACT_MIN_ENTRIES").as_deref() {
            Err(_) => 256,
            Ok(raw) => raw
                .parse::<i64>()
                .map_err(|e| invalid("LOKI_COMPACT_MIN_ENTRIES", e))
                .and_then(|n| {
                    if n >= 1 {
                        Ok(n)
                    } else {
                        Err(invalid("LOKI_COMPACT_MIN_ENTRIES", "must be >= 1"))
                    }
                })?,
        };

        let kek_b64 = required("LOKI_KEK_BASE64")?;
        let kek_bytes = base64_decode(&kek_b64)
            .ok_or_else(|| invalid("LOKI_KEK_BASE64", "not valid base64"))?;
        let kek = Kek::from_bytes(&kek_bytes).map_err(|e| invalid("LOKI_KEK_BASE64", e))?;

        Ok(Self {
            bind,
            database_url: required("DATABASE_URL")?,
            object_store,
            residency,
            default_tier,
            oidc_issuer: required("LOKI_OIDC_ISSUER")?,
            oidc_audience: required("LOKI_OIDC_AUDIENCE")?,
            oidc_keys,
            kek,
            compact_interval,
            compact_min_entries,
        })
    }
}

fn base64_decode(value: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(value.trim())
        .ok()
}
