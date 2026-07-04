// SPDX-License-Identifier: Apache-2.0

//! Public-key DEK wrapping for Tier 2 (zero-knowledge E2EE, ADR-C014).
//!
//! Sharing a Tier-2 document means re-wrapping its DEK to each member's
//! X25519 public key (ECIES-style: ephemeral X25519 → HKDF-SHA256 →
//! XChaCha20-Poly1305). The server stores only the wrapped form; unwrapping
//! happens on clients holding the member's secret key.

use hkdf::Hkdf;
use rand_core::OsRng;
use sha2::Sha256;
use x25519_dalek::{EphemeralSecret, PublicKey, StaticSecret};

use crate::dek::{DEK_LEN, Dek};
use crate::error::CryptoError;
use crate::wrap::{KeyWrap, WrappedDek};

/// Stable algorithm tag for this scheme.
pub const X25519_WRAP_ALGORITHM: &str = "x25519-hkdf-sha256-xchacha20.v1";

/// Byte length of an X25519 public key.
const PK_LEN: usize = 32;

/// A member's X25519 public key (stored in `app_user.public_key`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X25519PublicKey(PublicKey);

impl X25519PublicKey {
    /// Reconstructs a public key from its 32-byte form.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        let arr: [u8; PK_LEN] = bytes
            .try_into()
            .map_err(|_| CryptoError::InvalidKeyLength {
                expected: PK_LEN,
                actual: bytes.len(),
            })?;
        Ok(Self(PublicKey::from(arr)))
    }

    /// Returns the 32-byte wire form.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; PK_LEN] {
        self.0.as_bytes()
    }
}

/// A member's X25519 secret key (client-side only; never sent to the server).
pub struct X25519SecretKey(StaticSecret);

impl X25519SecretKey {
    /// Generates a fresh keypair secret.
    #[must_use]
    pub fn generate() -> Self {
        Self(StaticSecret::random_from_rng(OsRng))
    }

    /// Reconstructs a secret key from its 32-byte form.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
        let arr: [u8; PK_LEN] = bytes
            .try_into()
            .map_err(|_| CryptoError::InvalidKeyLength {
                expected: PK_LEN,
                actual: bytes.len(),
            })?;
        Ok(Self(StaticSecret::from(arr)))
    }

    /// Derives the matching public key.
    #[must_use]
    pub fn public_key(&self) -> X25519PublicKey {
        X25519PublicKey(PublicKey::from(&self.0))
    }
}

impl std::fmt::Debug for X25519SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("X25519SecretKey(..)")
    }
}

/// Wraps DEKs to an X25519 recipient; unwraps with the recipient's secret.
///
/// Blob layout: `ephemeral_pk (32 bytes) || nonce || ciphertext+tag`.
#[derive(Debug)]
pub struct X25519KeyWrap {
    recipient_pk: X25519PublicKey,
    recipient_sk: Option<X25519SecretKey>,
}

impl X25519KeyWrap {
    /// Wrap-only instance (all the server / a sharing client needs).
    #[must_use]
    pub fn for_recipient(recipient_pk: X25519PublicKey) -> Self {
        Self {
            recipient_pk,
            recipient_sk: None,
        }
    }

    /// Full instance for the key holder (a client unwrapping its own DEKs).
    #[must_use]
    pub fn for_key_holder(secret: X25519SecretKey) -> Self {
        Self {
            recipient_pk: secret.public_key(),
            recipient_sk: Some(secret),
        }
    }

    fn derive_wrapping_key(
        shared: &x25519_dalek::SharedSecret,
        ephemeral_pk: &PublicKey,
        recipient_pk: &X25519PublicKey,
    ) -> Result<Dek, CryptoError> {
        // Salt binds both public keys; info binds the algorithm tag.
        let mut salt = Vec::with_capacity(PK_LEN * 2);
        salt.extend_from_slice(ephemeral_pk.as_bytes());
        salt.extend_from_slice(recipient_pk.as_bytes());
        let hkdf = Hkdf::<Sha256>::new(Some(&salt), shared.as_bytes());
        let mut okm = [0u8; DEK_LEN];
        hkdf.expand(X25519_WRAP_ALGORITHM.as_bytes(), &mut okm)
            .map_err(|_| CryptoError::EncryptFailed)?;
        Dek::from_bytes(&okm)
    }
}

impl KeyWrap for X25519KeyWrap {
    fn algorithm(&self) -> &'static str {
        X25519_WRAP_ALGORITHM
    }

    fn wrap(&self, dek: &Dek) -> Result<WrappedDek, CryptoError> {
        let ephemeral = EphemeralSecret::random_from_rng(OsRng);
        let ephemeral_pk = PublicKey::from(&ephemeral);
        let shared = ephemeral.diffie_hellman(&self.recipient_pk.0);
        let wrapping_key = Self::derive_wrapping_key(&shared, &ephemeral_pk, &self.recipient_pk)?;
        let sealed = wrapping_key.seal(dek.as_bytes(), X25519_WRAP_ALGORITHM.as_bytes())?;
        let mut blob = Vec::with_capacity(PK_LEN + sealed.len());
        blob.extend_from_slice(ephemeral_pk.as_bytes());
        blob.extend_from_slice(&sealed);
        Ok(WrappedDek {
            algorithm: X25519_WRAP_ALGORITHM.to_owned(),
            blob,
        })
    }

    fn unwrap_dek(&self, wrapped: &WrappedDek) -> Result<Dek, CryptoError> {
        if wrapped.algorithm != X25519_WRAP_ALGORITHM {
            return Err(CryptoError::UnsupportedAlgorithm(wrapped.algorithm.clone()));
        }
        let Some(secret) = &self.recipient_sk else {
            // A wrap-only instance (e.g. the server) cannot unwrap — that is
            // the zero-knowledge property, expressed as a decrypt failure.
            return Err(CryptoError::DecryptFailed);
        };
        if wrapped.blob.len() < PK_LEN {
            return Err(CryptoError::CiphertextTooShort(wrapped.blob.len()));
        }
        let (pk_bytes, sealed) = wrapped.blob.split_at(PK_LEN);
        let ephemeral_pk = X25519PublicKey::from_bytes(pk_bytes)?;
        let shared = secret.0.diffie_hellman(&ephemeral_pk.0);
        let wrapping_key = Self::derive_wrapping_key(&shared, &ephemeral_pk.0, &self.recipient_pk)?;
        let raw = wrapping_key.open(sealed, X25519_WRAP_ALGORITHM.as_bytes())?;
        Dek::from_bytes(&raw)
    }
}

#[cfg(test)]
#[path = "x25519_wrap_tests.rs"]
mod tests;
