// SPDX-License-Identifier: Apache-2.0

//! Envelope encryption and crypto-agile key wrapping (ADR-C014).
//!
//! Every document has a per-document **DEK** (data-encryption key). [`Dek::seal`]
//! seals content with the DEK via XChaCha20-Poly1305 — the primitive intended
//! for **Tier-2, client-side** content encryption; the server does not seal
//! Tier-0/1 content (it sees plaintext by design, ADR-C014), so on the server
//! the DEK is used only for key-wrapping and crypto-shred metadata. The DEK is
//! wrapped by a tier-specific **KEK** through the [`KeyWrap`] trait:
//!
//! - Tier 0/1: a symmetric KEK ([`AeadKeyWrap`]) held by the platform KMS
//!   (Tier 0) or the customer's KMS/HSM (Tier 1).
//! - Tier 2: the recipient's X25519 public key ([`X25519KeyWrap`]) — the
//!   server only ever sees the wrapped form.
//!
//! [`WrappedDek`] records the wrapping algorithm alongside the blob so hybrid
//! PQC (X25519 + ML-KEM) can be introduced later without a data migration
//! (crypto-agility, C5:2026).

#![forbid(unsafe_code)]

mod aead_wrap;
mod dek;
mod error;
mod wrap;
mod x25519_wrap;

#[cfg(test)]
mod vectors_tests;

pub use aead_wrap::{AeadKeyWrap, Kek};
pub use dek::{DEK_LEN, Dek};
pub use error::CryptoError;
pub use wrap::{KeyWrap, WrappedDek};
pub use x25519_wrap::{X25519KeyWrap, X25519PublicKey, X25519SecretKey};
