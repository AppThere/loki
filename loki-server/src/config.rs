// SPDX-License-Identifier: Apache-2.0

//! Environment-driven configuration (ADR-C018: env + optional file; no
//! phone-home, telemetry off by default).
//!
//! Sovereignty checks happen here, at validation time (ADR-C019): the
//! residency value must be an allowed EU region or an explicit
//! `self-hosted:<label>`, and the deployment default tier can never be
//! Tier 2 (zero-knowledge is per-document opt-in, ratified decision §6.1).

use std::net::SocketAddr;

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
    /// PEM-encoded RSA public key for token verification
    /// (`LOKI_OIDC_RSA_PEM_FILE`, a file path).
    // TODO(oidc-jwks): replace with JWKS discovery + rotation (see
    // loki-server-auth).
    pub oidc_rsa_pem: Vec<u8>,
    /// Tier-0/1 key-encryption key (`LOKI_KEK_BASE64`, 32 bytes base64).
    // TODO(kms): source the KEK from Vault/KMS/PKCS#11 instead of the
    // environment; env is acceptable for single-node self-host only.
    pub kek: Kek,
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

        let pem_path = required("LOKI_OIDC_RSA_PEM_FILE")?;
        let oidc_rsa_pem =
            std::fs::read(&pem_path).map_err(|e| invalid("LOKI_OIDC_RSA_PEM_FILE", e))?;

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
            oidc_rsa_pem,
            kek,
        })
    }
}

fn base64_decode(value: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(value.trim())
        .ok()
}
