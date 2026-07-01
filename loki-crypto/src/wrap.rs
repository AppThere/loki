// SPDX-License-Identifier: Apache-2.0

//! The crypto-agile key-wrapping port (ADR-C014).

use serde::{Deserialize, Serialize};

use crate::dek::Dek;
use crate::error::CryptoError;

/// A wrapped (encrypted) DEK, tagged with the algorithm that wrapped it.
///
/// The `algorithm` tag is stored alongside the blob (in `doc_meta.dek_wrapped`
/// / `doc_member.dek_wrapped_for_user`) so a future hybrid-PQC wrap
/// (X25519 + ML-KEM) can coexist with current data — unwrapping dispatches on
/// the tag instead of assuming one scheme.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WrappedDek {
    /// Identifier of the wrapping scheme, e.g. `"xchacha20-poly1305-kek.v1"`.
    pub algorithm: String,
    /// Opaque wrapped key material.
    #[serde(with = "serde_bytes_base64")]
    pub blob: Vec<u8>,
}

/// A scheme that can wrap and unwrap document DEKs.
///
/// Implementations: [`crate::AeadKeyWrap`] (symmetric KEK, Tiers 0/1) and
/// [`crate::X25519KeyWrap`] (per-member public key, Tier 2). A hybrid PQC
/// implementation slots in without touching stored data.
pub trait KeyWrap {
    /// Stable identifier written into [`WrappedDek::algorithm`].
    fn algorithm(&self) -> &'static str;

    /// Wraps `dek` for the key custodian this instance represents.
    fn wrap(&self, dek: &Dek) -> Result<WrappedDek, CryptoError>;

    /// Unwraps a DEK previously produced by [`KeyWrap::wrap`].
    ///
    /// Returns [`CryptoError::UnsupportedAlgorithm`] if `wrapped.algorithm`
    /// does not match this scheme.
    fn unwrap_dek(&self, wrapped: &WrappedDek) -> Result<Dek, CryptoError>;
}

/// Serializes the wrapped blob as base64 for JSON transport.
mod serde_bytes_base64 {
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(deserializer)?;
        STANDARD.decode(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapped_dek_json_round_trip() {
        let wrapped = WrappedDek {
            algorithm: "xchacha20-poly1305-kek.v1".to_owned(),
            blob: (0..64u8).collect(),
        };
        let json = serde_json::to_string(&wrapped).unwrap();
        let back: WrappedDek = serde_json::from_str(&json).unwrap();
        assert_eq!(back, wrapped);
    }
}
