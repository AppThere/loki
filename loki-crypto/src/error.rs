// SPDX-License-Identifier: Apache-2.0

//! Typed errors for envelope encryption and key wrapping.

/// Errors produced by sealing, opening, wrapping, or unwrapping keys.
///
/// AEAD failures are deliberately opaque (no distinction between a bad key,
/// a truncated ciphertext, and a forged tag) to avoid oracle behaviour.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    /// Authenticated decryption failed (wrong key, tampered or truncated data).
    #[error("authenticated decryption failed")]
    DecryptFailed,
    /// Authenticated encryption failed (should not happen with valid inputs).
    #[error("authenticated encryption failed")]
    EncryptFailed,
    /// The ciphertext blob is too short to contain a nonce.
    #[error("ciphertext too short ({0} bytes)")]
    CiphertextTooShort(usize),
    /// A wrapped DEK declared an algorithm this build does not implement.
    #[error("unsupported key-wrap algorithm {0:?}")]
    UnsupportedAlgorithm(String),
    /// A key blob had the wrong length.
    #[error("invalid key length {actual} (expected {expected})")]
    InvalidKeyLength {
        /// Expected byte length.
        expected: usize,
        /// Actual byte length received.
        actual: usize,
    },
}
