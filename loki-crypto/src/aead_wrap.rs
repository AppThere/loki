// SPDX-License-Identifier: Apache-2.0

//! Symmetric-KEK key wrapping for Tiers 0 and 1 (ADR-C014).

use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::dek::{DEK_LEN, Dek};
use crate::error::CryptoError;
use crate::wrap::{KeyWrap, WrappedDek};

/// Stable algorithm tag for this scheme.
pub const AEAD_WRAP_ALGORITHM: &str = "xchacha20-poly1305-kek.v1";

/// A symmetric key-encryption key.
///
/// Under Tier 0 the KEK lives in the platform KMS; under Tier 1 it is
/// customer-managed (Vault, PKCS#11). In both cases the raw KEK bytes enter
/// this process only inside the relevant trust boundary.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Kek([u8; DEK_LEN]);

impl Kek {
    /// Reconstructs a KEK from raw bytes supplied by the KMS integration.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        let arr: [u8; DEK_LEN] = bytes
            .try_into()
            .map_err(|_| CryptoError::InvalidKeyLength {
                expected: DEK_LEN,
                actual: bytes.len(),
            })?;
        Ok(Self(arr))
    }

    /// Generates a fresh random KEK (used by tests and local-dev setups).
    #[must_use]
    pub fn generate() -> Self {
        Self(*Dek::generate().as_bytes())
    }
}

impl std::fmt::Debug for Kek {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Kek(..)")
    }
}

/// Wraps DEKs under a symmetric KEK with XChaCha20-Poly1305.
#[derive(Debug)]
pub struct AeadKeyWrap {
    kek: Kek,
}

impl AeadKeyWrap {
    /// Creates a wrapper around the given KEK.
    #[must_use]
    pub fn new(kek: Kek) -> Self {
        Self { kek }
    }
}

impl KeyWrap for AeadKeyWrap {
    fn algorithm(&self) -> &'static str {
        AEAD_WRAP_ALGORITHM
    }

    fn wrap(&self, dek: &Dek) -> Result<WrappedDek, CryptoError> {
        // The KEK is structurally a 256-bit XChaCha20 key, so reuse Dek's
        // sealed-blob layout (nonce || ciphertext+tag) with the algorithm
        // tag as AAD.
        let kek_cipher = Dek::from_bytes(&self.kek.0)?;
        let blob = kek_cipher.seal(dek.as_bytes(), AEAD_WRAP_ALGORITHM.as_bytes())?;
        Ok(WrappedDek {
            algorithm: AEAD_WRAP_ALGORITHM.to_owned(),
            blob,
        })
    }

    fn unwrap_dek(&self, wrapped: &WrappedDek) -> Result<Dek, CryptoError> {
        if wrapped.algorithm != AEAD_WRAP_ALGORITHM {
            return Err(CryptoError::UnsupportedAlgorithm(wrapped.algorithm.clone()));
        }
        let kek_cipher = Dek::from_bytes(&self.kek.0)?;
        let raw = kek_cipher.open(&wrapped.blob, AEAD_WRAP_ALGORITHM.as_bytes())?;
        Dek::from_bytes(&raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_unwrap_round_trip() {
        let wrapper = AeadKeyWrap::new(Kek::generate());
        let dek = Dek::generate();
        let wrapped = wrapper.wrap(&dek).unwrap();
        assert_eq!(wrapped.algorithm, AEAD_WRAP_ALGORITHM);
        let unwrapped = wrapper.unwrap_dek(&wrapped).unwrap();
        assert_eq!(unwrapped.as_bytes(), dek.as_bytes());
    }

    #[test]
    fn wrong_kek_fails() {
        let wrapper = AeadKeyWrap::new(Kek::generate());
        let other = AeadKeyWrap::new(Kek::generate());
        let wrapped = wrapper.wrap(&Dek::generate()).unwrap();
        assert!(other.unwrap_dek(&wrapped).is_err());
    }

    #[test]
    fn algorithm_mismatch_is_typed() {
        let wrapper = AeadKeyWrap::new(Kek::generate());
        let mut wrapped = wrapper.wrap(&Dek::generate()).unwrap();
        wrapped.algorithm = "x25519-xchacha20.v1".to_owned();
        assert!(matches!(
            wrapper.unwrap_dek(&wrapped),
            Err(CryptoError::UnsupportedAlgorithm(_))
        ));
    }
}
