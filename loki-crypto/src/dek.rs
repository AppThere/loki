// SPDX-License-Identifier: Apache-2.0

//! Per-document data-encryption keys and AEAD sealing.

use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand_core::OsRng;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::error::CryptoError;

/// Byte length of a DEK (XChaCha20-Poly1305 key).
pub const DEK_LEN: usize = 32;

/// XChaCha20-Poly1305 nonce length; the nonce is prepended to ciphertext.
const NONCE_LEN: usize = 24;

/// A per-document data-encryption key.
///
/// The key material is zeroized on drop. Destroying every wrapped copy of a
/// DEK crypto-shreds the document (GDPR erasure, ADR-C020).
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Dek([u8; DEK_LEN]);

impl Dek {
    /// Generates a fresh random DEK.
    #[must_use]
    pub fn generate() -> Self {
        let key = XChaCha20Poly1305::generate_key(&mut OsRng);
        Self(key.into())
    }

    /// Reconstructs a DEK from raw bytes (e.g. after unwrapping).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        let arr: [u8; DEK_LEN] = bytes
            .try_into()
            .map_err(|_| CryptoError::InvalidKeyLength {
                expected: DEK_LEN,
                actual: bytes.len(),
            })?;
        Ok(Self(arr))
    }

    /// Exposes the raw key bytes (needed to wrap the DEK; handle with care).
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; DEK_LEN] {
        &self.0
    }

    /// Seals `plaintext` with this DEK, binding `aad` (e.g. the document id).
    ///
    /// Output layout: `nonce (24 bytes) || ciphertext+tag`. A fresh random
    /// nonce is used per call; XChaCha's 192-bit nonce makes random nonces
    /// collision-safe at any realistic volume.
    pub fn seal(&self, plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let cipher = XChaCha20Poly1305::new(self.0.as_slice().into());
        let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| CryptoError::EncryptFailed)?;
        let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    /// Opens a blob produced by [`Dek::seal`] with the same `aad`.
    pub fn open(&self, sealed: &[u8], aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
        if sealed.len() < NONCE_LEN {
            return Err(CryptoError::CiphertextTooShort(sealed.len()));
        }
        let (nonce, ciphertext) = sealed.split_at(NONCE_LEN);
        let cipher = XChaCha20Poly1305::new(self.0.as_slice().into());
        cipher
            .decrypt(
                XNonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad,
                },
            )
            .map_err(|_| CryptoError::DecryptFailed)
    }
}

impl std::fmt::Debug for Dek {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print key material.
        f.write_str("Dek(..)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open_round_trip() {
        let dek = Dek::generate();
        let sealed = dek.seal(b"hello loki", b"doc-1").unwrap();
        assert_eq!(dek.open(&sealed, b"doc-1").unwrap(), b"hello loki");
    }

    #[test]
    fn wrong_key_or_aad_fails() {
        let dek = Dek::generate();
        let other = Dek::generate();
        let sealed = dek.seal(b"secret", b"doc-1").unwrap();
        assert!(matches!(
            other.open(&sealed, b"doc-1"),
            Err(CryptoError::DecryptFailed)
        ));
        assert!(matches!(
            dek.open(&sealed, b"doc-2"),
            Err(CryptoError::DecryptFailed)
        ));
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let dek = Dek::generate();
        let mut sealed = dek.seal(b"secret", b"").unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 0x01;
        assert!(dek.open(&sealed, b"").is_err());
        assert!(matches!(
            dek.open(&sealed[..10], b""),
            Err(CryptoError::CiphertextTooShort(10))
        ));
    }

    #[test]
    fn debug_never_leaks_key_material() {
        let dek = Dek::generate();
        assert_eq!(format!("{dek:?}"), "Dek(..)");
    }
}
