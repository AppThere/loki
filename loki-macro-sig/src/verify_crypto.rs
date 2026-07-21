// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Digest- and signature-algorithm agility for the CMS verifier (8A.3).
//!
//! Maps the OIDs carried in a `SignerInfo` to a concrete digest ([`DigestId`])
//! and public-key family ([`SigKind`]), computes content digests, and performs
//! the actual RSA PKCS#1 v1.5 / ECDSA P-256 signature check. Everything here is
//! total on hostile input: a decode or verification failure is a `false` /
//! `None`, never a panic (T9).

use const_oid::db::rfc5912::{
    ECDSA_WITH_SHA_224, ECDSA_WITH_SHA_256, ECDSA_WITH_SHA_384, ECDSA_WITH_SHA_512,
    ID_EC_PUBLIC_KEY, ID_MD_5, ID_SHA_1, ID_SHA_256, ID_SHA_384, ID_SHA_512,
    MD_5_WITH_RSA_ENCRYPTION, RSA_ENCRYPTION, SHA_1_WITH_RSA_ENCRYPTION,
    SHA_256_WITH_RSA_ENCRYPTION, SHA_384_WITH_RSA_ENCRYPTION, SHA_512_WITH_RSA_ENCRYPTION,
};
use der::asn1::ObjectIdentifier;
use md5::Md5;
use rsa::RsaPublicKey;
use rsa::pkcs1v15::{Signature as RsaSignature, VerifyingKey as RsaVerifyingKey};
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha384, Sha512};
use signature::Verifier;
use signature::hazmat::PrehashVerifier;
use spki::DecodePublicKey;

/// A digest algorithm the verifier can compute and check.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum DigestId {
    /// SHA-256 — trust-eligible.
    Sha256,
    /// SHA-384 — trust-eligible.
    Sha384,
    /// SHA-512 — trust-eligible.
    Sha512,
    /// SHA-1 — cryptographically broken; checked but only ever `ValidUntrusted`.
    Sha1,
    /// MD5 — cryptographically broken; checked but only ever `ValidUntrusted`.
    Md5,
}

impl DigestId {
    /// Maps a `digestAlgorithm` OID to a [`DigestId`], or `None` for one this
    /// crate does not implement.
    pub(crate) fn from_oid(oid: &ObjectIdentifier) -> Option<Self> {
        let o = *oid;
        if o == ID_SHA_256 {
            Some(DigestId::Sha256)
        } else if o == ID_SHA_384 {
            Some(DigestId::Sha384)
        } else if o == ID_SHA_512 {
            Some(DigestId::Sha512)
        } else if o == ID_SHA_1 {
            Some(DigestId::Sha1)
        } else if o == ID_MD_5 {
            Some(DigestId::Md5)
        } else {
            None
        }
    }

    /// Whether this is a legacy (broken) digest that must never grant trust — the
    /// downgrade defence (ADR-0014 §4.2).
    pub(crate) fn is_legacy(self) -> bool {
        matches!(self, DigestId::Sha1 | DigestId::Md5)
    }

    /// The digest of `data` under this algorithm.
    pub(crate) fn digest(self, data: &[u8]) -> Vec<u8> {
        match self {
            DigestId::Sha256 => Sha256::digest(data).to_vec(),
            DigestId::Sha384 => Sha384::digest(data).to_vec(),
            DigestId::Sha512 => Sha512::digest(data).to_vec(),
            DigestId::Sha1 => Sha1::digest(data).to_vec(),
            DigestId::Md5 => Md5::digest(data).to_vec(),
        }
    }
}

/// The public-key family of a `signatureAlgorithm`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SigKind {
    /// RSA PKCS#1 v1.5.
    Rsa,
    /// ECDSA (P-256 curve).
    Ecdsa,
}

impl SigKind {
    /// Maps a `signatureAlgorithm` OID to a [`SigKind`], or `None` for one this
    /// crate does not implement. Accepts both the bare key OIDs
    /// (`rsaEncryption`, `id-ecPublicKey`) and the combined
    /// `*WithRSA`/`ecdsa-with-*` forms real signers emit.
    pub(crate) fn from_oid(oid: &ObjectIdentifier) -> Option<Self> {
        let o = *oid;
        if o == RSA_ENCRYPTION
            || o == SHA_256_WITH_RSA_ENCRYPTION
            || o == SHA_384_WITH_RSA_ENCRYPTION
            || o == SHA_512_WITH_RSA_ENCRYPTION
            || o == SHA_1_WITH_RSA_ENCRYPTION
            || o == MD_5_WITH_RSA_ENCRYPTION
        {
            Some(SigKind::Rsa)
        } else if o == ID_EC_PUBLIC_KEY
            || o == ECDSA_WITH_SHA_256
            || o == ECDSA_WITH_SHA_384
            || o == ECDSA_WITH_SHA_512
            || o == ECDSA_WITH_SHA_224
        {
            Some(SigKind::Ecdsa)
        } else {
            None
        }
    }
}

/// Verifies `signature` over `message` (already the exact signed bytes — the
/// re-encoded `signedAttrs` or the raw content) using the signer's DER-encoded
/// `SubjectPublicKeyInfo`. Returns `true` only on a cryptographically valid
/// signature; every parse/verify failure is `false`.
pub(crate) fn verify_signature(
    kind: SigKind,
    digest: DigestId,
    spki_der: &[u8],
    message: &[u8],
    signature: &[u8],
) -> bool {
    match kind {
        SigKind::Rsa => verify_rsa(digest, spki_der, message, signature),
        SigKind::Ecdsa => verify_ecdsa_p256(digest, spki_der, message, signature),
    }
}

/// RSA PKCS#1 v1.5 verification. The `DigestInfo` prefix depends on the digest,
/// so each arm monomorphises the verifying key for its hash.
fn verify_rsa(digest: DigestId, spki_der: &[u8], message: &[u8], signature: &[u8]) -> bool {
    let Ok(public_key) = RsaPublicKey::from_public_key_der(spki_der) else {
        return false;
    };
    let Ok(sig) = RsaSignature::try_from(signature) else {
        return false;
    };
    match digest {
        DigestId::Sha256 => RsaVerifyingKey::<Sha256>::new(public_key)
            .verify(message, &sig)
            .is_ok(),
        DigestId::Sha384 => RsaVerifyingKey::<Sha384>::new(public_key)
            .verify(message, &sig)
            .is_ok(),
        DigestId::Sha512 => RsaVerifyingKey::<Sha512>::new(public_key)
            .verify(message, &sig)
            .is_ok(),
        DigestId::Sha1 => RsaVerifyingKey::<Sha1>::new(public_key)
            .verify(message, &sig)
            .is_ok(),
        DigestId::Md5 => RsaVerifyingKey::<Md5>::new(public_key)
            .verify(message, &sig)
            .is_ok(),
    }
}

/// ECDSA P-256 verification. The digest is applied by us (prehash) so any
/// supported hash pairs with the fixed curve; the signature is DER-encoded.
fn verify_ecdsa_p256(digest: DigestId, spki_der: &[u8], message: &[u8], signature: &[u8]) -> bool {
    let Ok(verifying_key) = p256::ecdsa::VerifyingKey::from_public_key_der(spki_der) else {
        return false;
    };
    let Ok(sig) = p256::ecdsa::Signature::from_der(signature) else {
        return false;
    };
    let prehash = digest.digest(message);
    verifying_key.verify_prehash(&prehash, &sig).is_ok()
}

#[cfg(test)]
#[path = "verify_crypto_tests.rs"]
mod tests;
