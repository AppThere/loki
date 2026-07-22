// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Detached CMS `SignedData` + X.509 verification (8A.3; ADR-0014 §4.1–§4.4).
//!
//! [`verify_signed_data`] takes a DER PKCS#7 `SignedData` blob (as located by
//! [`crate::RawVbaSignature::pkcs7_der`]) and the `content` bytes it should
//! cover, and returns a total [`SignatureVerdict`]. It proves **integrity +
//! authorship** only; it never returns [`SignatureVerdict::ValidTrusted`], since
//! trust is a caller decision against a user-pinned publisher store (8A.5). A
//! fully-valid signature is therefore [`UntrustedReason::NotPinned`] here.
//!
//! The one CMS subtlety that bites every implementation: when `signedAttrs` are
//! present the signature is computed over the DER re-encoding of the attributes
//! **with the universal `SET OF` tag (`0x31`)**, not the `[0] IMPLICIT` tag they
//! carry inside `SignerInfo`. The `cms` crate stores `signedAttrs` as a
//! `SetOfVec`, so `to_der()` on it yields exactly that `0x31` re-encoding
//! (RFC 5652 §5.4).
//!
//! TODO(8A.3-corpus): the caller supplies `content`; wiring the exact MS-OVBA
//! V3/agile signed-content hash and cross-checking this verifier against a
//! corpus of real signed `.docm`/`.odt` samples is the remaining corpus step
//! (ADR-0014 §6, gate on 8A.3).

use std::time::{SystemTime, UNIX_EPOCH};

use cms::content_info::ContentInfo;
use cms::signed_data::{SignedAttributes, SignedData};
use const_oid::db::rfc5911::{ID_MESSAGE_DIGEST, ID_SIGNED_DATA};
use der::asn1::OctetString;
use der::{Decode, Encode};

use crate::verdict::{CertInfo, InvalidReason, SignatureVerdict, UntrustedReason};
use crate::verify_cert::{cert_info, signer_cert};
use crate::verify_crypto::{DigestId, EcdsaEncoding, SigKind, verify_signature};

/// Verifies a detached CMS `SignedData` (`pkcs7_der`) over `content`.
///
/// Total: every parse or cryptographic failure maps to
/// [`SignatureVerdict::Invalid`], never a panic. See the module docs for the
/// verdict semantics; this layer never grants trust.
#[must_use]
pub fn verify_signed_data(pkcs7_der: &[u8], content: &[u8]) -> SignatureVerdict {
    match verify_inner(pkcs7_der, content) {
        Ok(verdict) => verdict,
        Err(reason) => SignatureVerdict::Invalid(reason),
    }
}

/// The fallible core: `Ok` carries a `ValidUntrusted` verdict, `Err` a reason the
/// caller wraps in [`SignatureVerdict::Invalid`].
fn verify_inner(pkcs7_der: &[u8], content: &[u8]) -> Result<SignatureVerdict, InvalidReason> {
    let content_info = ContentInfo::from_der(pkcs7_der).map_err(|_| InvalidReason::Malformed)?;
    if content_info.content_type != ID_SIGNED_DATA {
        return Err(InvalidReason::Malformed);
    }
    let signed_data: SignedData = content_info
        .content
        .decode_as()
        .map_err(|_| InvalidReason::Malformed)?;

    // First SignerInfo (VBA/Office emit exactly one).
    let signer = signed_data
        .signer_infos
        .0
        .get(0)
        .ok_or(InvalidReason::Malformed)?;

    let digest =
        DigestId::from_oid(&signer.digest_alg.oid).ok_or(InvalidReason::UnsupportedAlgorithm)?;
    let sig_kind = SigKind::from_oid(&signer.signature_algorithm.oid)
        .ok_or(InvalidReason::UnsupportedAlgorithm)?;

    let cert = signer_cert(&signed_data, &signer.sid).ok_or(InvalidReason::Malformed)?;
    let info = cert_info(cert).ok_or(InvalidReason::Malformed)?;
    let spki_der = cert
        .tbs_certificate
        .subject_public_key_info
        .to_der()
        .map_err(|_| InvalidReason::Malformed)?;

    let signature = signer.signature.as_bytes();
    let verified = match &signer.signed_attrs {
        Some(attrs) => {
            // signedAttrs MUST carry a message-digest equal to digest(content).
            let message_digest =
                message_digest_attr(attrs).ok_or(InvalidReason::ContentMismatch)?;
            if message_digest != digest.digest(content) {
                return Err(InvalidReason::ContentMismatch);
            }
            // Signature is over the 0x31-tagged re-encoding of signedAttrs.
            let signed_bytes = attrs.to_der().map_err(|_| InvalidReason::Malformed)?;
            verify_signature(
                sig_kind,
                digest,
                &spki_der,
                &signed_bytes,
                signature,
                EcdsaEncoding::Der,
            )
        }
        None => verify_signature(
            sig_kind,
            digest,
            &spki_der,
            content,
            signature,
            EcdsaEncoding::Der,
        ),
    };
    if !verified {
        return Err(InvalidReason::DigestMismatch);
    }

    // An RFC-3161 timestamp bound to this signature rescues an expired cert
    // (ADR-0014 §4.4): if it proves the signature predates expiry, the cert
    // counts as valid-at-signing.
    let signed_time = crate::timestamp::signed_time(signer, signature);

    Ok(SignatureVerdict::ValidUntrusted {
        reason: untrusted_reason(digest, &info, signed_time),
        signer: info,
    })
}

/// Picks the `ValidUntrusted` reason with the ADR-mandated precedence: a legacy
/// (broken) digest wins, then an expired certificate (unless a trusted timestamp
/// proves it was signed while valid), else plain `NotPinned`. Shared with the ODF
/// `XMLDSig` verifier (8A.4), which passes `signed_time = None`.
pub(crate) fn untrusted_reason(
    digest: DigestId,
    info: &CertInfo,
    signed_time: Option<i64>,
) -> UntrustedReason {
    if digest.is_legacy() {
        UntrustedReason::LegacyAlgorithm
    } else if is_expired_at(info, signed_time) {
        UntrustedReason::CertificateExpired
    } else {
        UntrustedReason::NotPinned
    }
}

/// Whether the certificate is expired **for trust purposes**: its `notAfter` is
/// in the past *and* no bound timestamp proves the signature predates expiry. A
/// `signed_time` within `[notBefore, notAfter]` means the certificate was valid
/// when it signed, so expiry no longer withholds trust (ADR-0014 §4.4).
fn is_expired_at(info: &CertInfo, signed_time: Option<i64>) -> bool {
    if let Some(t) = signed_time
        && t >= info.not_before
        && t <= info.not_after
    {
        return false;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0);
    now > info.not_after
}

/// The `message-digest` (OID 1.2.840.113549.1.9.4) attribute value, if present.
fn message_digest_attr(attrs: &SignedAttributes) -> Option<Vec<u8>> {
    let attr = attrs.iter().find(|a| a.oid == ID_MESSAGE_DIGEST)?;
    let value = attr.values.get(0)?;
    let octet = value.decode_as::<OctetString>().ok()?;
    Some(octet.into_bytes())
}

#[cfg(test)]
#[path = "verify_tests.rs"]
mod tests;
