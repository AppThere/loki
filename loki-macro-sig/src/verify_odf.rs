// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODF `XMLDSig` verification over a `macrosignatures.xml` (8A.4; ADR-0014 §4.5).
//!
//! [`verify_xmldsig`] parses the signature document ([`crate::odf`]), and for
//! each `Signature`:
//!
//! 1. resolves the `SignatureMethod` / `DigestMethod` algorithm URIs;
//! 2. checks every **package-part** `Reference` digest against the actual part
//!    bytes (supplied by `resolve_part`) — this is the macro-integrity proof;
//! 3. canonicalises `SignedInfo` (inclusive C14N) and verifies the
//!    `SignatureValue` against the signer certificate's public key;
//! 4. extracts the signer [`CertInfo`] and returns a total [`SignatureVerdict`].
//!
//! Like the CMS path it proves **integrity + authorship, never trust** — a valid
//! signature is `ValidUntrusted` (`NotPinned` / `LegacyAlgorithm` /
//! `CertificateExpired`); only the caller's pinned-publisher store yields
//! `ValidTrusted` (8A.5, T10).
//!
//! TODO(8A.4-corpus): in-document fragment references (`URI="#..."`, the signing
//! -time `Object`) are authenticated by the `SignedInfo` signature but their
//! digests are not independently re-canonicalised here — they carry signing
//! metadata, not macro source. Cross-validating the whole path against real
//! `LibreOffice` `.odt`/`.ods` signatures is the remaining gate (ADR-0014 §6).

use x509_cert::Certificate;
use x509_cert::der::{Decode, Encode};

use crate::odf::{OdfReference, OdfSignature, parse_macro_signatures};
use crate::verdict::{InvalidReason, SignatureVerdict};
use crate::verify::untrusted_reason;
use crate::verify_cert::cert_info;
use crate::verify_crypto::{DigestId, EcdsaEncoding, SigKind, verify_signature};
use crate::xml_c14n::canonicalize;

/// Verifies the macro signatures in a `macrosignatures.xml`. `resolve_part` maps a
/// reference URI (a package path such as `Basic/Standard/Module1.xml`) to the
/// exact bytes Loki would hash; returning `None` means the signed part is missing
/// and the signature fails closed.
///
/// Returns [`SignatureVerdict::Unsigned`] when the document holds no signature,
/// and otherwise the strongest verdict across the signatures present.
pub fn verify_xmldsig<F>(macrosignatures_xml: &[u8], resolve_part: F) -> SignatureVerdict
where
    F: Fn(&str) -> Option<Vec<u8>>,
{
    let signatures = parse_macro_signatures(macrosignatures_xml);
    if signatures.is_empty() {
        return SignatureVerdict::Unsigned;
    }
    let mut best: Option<SignatureVerdict> = None;
    for sig in &signatures {
        let verdict = verify_one(sig, &resolve_part);
        best = Some(match best.take() {
            Some(current) => stronger(current, verdict),
            None => verdict,
        });
    }
    best.unwrap_or(SignatureVerdict::Unsigned)
}

/// Verifies a single parsed signature.
fn verify_one<F>(sig: &OdfSignature, resolve_part: &F) -> SignatureVerdict
where
    F: Fn(&str) -> Option<Vec<u8>>,
{
    match verify_one_inner(sig, resolve_part) {
        Ok(verdict) => verdict,
        Err(reason) => SignatureVerdict::Invalid(reason),
    }
}

fn verify_one_inner<F>(
    sig: &OdfSignature,
    resolve_part: &F,
) -> Result<SignatureVerdict, InvalidReason>
where
    F: Fn(&str) -> Option<Vec<u8>>,
{
    let (sig_kind, sig_digest) =
        sig_method(&sig.signature_method_uri).ok_or(InvalidReason::UnsupportedAlgorithm)?;

    let cert = Certificate::from_der(&sig.cert_der).map_err(|_| InvalidReason::Malformed)?;
    let info = cert_info(&cert).ok_or(InvalidReason::Malformed)?;
    let spki_der = cert
        .tbs_certificate
        .subject_public_key_info
        .to_der()
        .map_err(|_| InvalidReason::Malformed)?;

    // Each package-part reference digest must match the real part bytes.
    for reference in &sig.references {
        check_reference(reference, resolve_part)?;
    }

    // The signature is over the canonical SignedInfo octet stream.
    let canonical = canonicalize(&sig.signed_info);
    let verified = verify_signature(
        sig_kind,
        sig_digest,
        &spki_der,
        &canonical,
        &sig.signature_value,
        EcdsaEncoding::P1363,
    );
    if !verified {
        return Err(InvalidReason::DigestMismatch);
    }

    // ODF/XAdES timestamps are a different structure than CMS unsigned attrs
    // (TODO(8A.6-xades): honour `xades:SignatureTimeStamp`); none is consulted
    // here, so expiry is judged against the clock alone.
    Ok(SignatureVerdict::ValidUntrusted {
        reason: untrusted_reason(sig_digest, &info, None),
        signer: info,
    })
}

/// Checks one reference's digest against the resolved part bytes. In-document
/// fragment references (`#...`) are covered by the `SignedInfo` signature and not
/// re-hashed here (see the module `TODO`).
fn check_reference<F>(reference: &OdfReference, resolve_part: &F) -> Result<(), InvalidReason>
where
    F: Fn(&str) -> Option<Vec<u8>>,
{
    if reference.uri.starts_with('#') {
        return Ok(());
    }
    let digest =
        digest_method(&reference.digest_alg_uri).ok_or(InvalidReason::UnsupportedAlgorithm)?;
    let bytes = resolve_part(&reference.uri).ok_or(InvalidReason::ContentMismatch)?;
    if digest.digest(&bytes) != reference.digest_value {
        return Err(InvalidReason::ContentMismatch);
    }
    Ok(())
}

/// Ranks two verdicts, keeping the stronger: `ValidTrusted` > `ValidUntrusted` >
/// `Invalid` > `Unsigned`. (This crate never yields `ValidTrusted`.)
fn stronger(a: SignatureVerdict, b: SignatureVerdict) -> SignatureVerdict {
    if rank(&b) > rank(&a) { b } else { a }
}

fn rank(v: &SignatureVerdict) -> u8 {
    match v {
        SignatureVerdict::Unsigned => 0,
        SignatureVerdict::Invalid(_) => 1,
        SignatureVerdict::ValidUntrusted { .. } => 2,
        SignatureVerdict::ValidTrusted { .. } => 3,
    }
}

/// Maps an `XMLDSig` `SignatureMethod` algorithm URI to `(key family, digest)`.
fn sig_method(uri: &str) -> Option<(SigKind, DigestId)> {
    Some(match uri {
        "http://www.w3.org/2000/09/xmldsig#rsa-sha1" => (SigKind::Rsa, DigestId::Sha1),
        "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256" => (SigKind::Rsa, DigestId::Sha256),
        "http://www.w3.org/2001/04/xmldsig-more#rsa-sha384" => (SigKind::Rsa, DigestId::Sha384),
        "http://www.w3.org/2001/04/xmldsig-more#rsa-sha512" => (SigKind::Rsa, DigestId::Sha512),
        "http://www.w3.org/2000/09/xmldsig#ecdsa-sha1"
        | "http://www.w3.org/2001/04/xmldsig-more#ecdsa-sha1" => (SigKind::Ecdsa, DigestId::Sha1),
        "http://www.w3.org/2001/04/xmldsig-more#ecdsa-sha256" => (SigKind::Ecdsa, DigestId::Sha256),
        "http://www.w3.org/2001/04/xmldsig-more#ecdsa-sha384" => (SigKind::Ecdsa, DigestId::Sha384),
        "http://www.w3.org/2001/04/xmldsig-more#ecdsa-sha512" => (SigKind::Ecdsa, DigestId::Sha512),
        _ => return None,
    })
}

/// Maps an `XMLDSig` `DigestMethod` algorithm URI to a digest.
fn digest_method(uri: &str) -> Option<DigestId> {
    Some(match uri {
        "http://www.w3.org/2001/04/xmlenc#sha256" => DigestId::Sha256,
        "http://www.w3.org/2001/04/xmldsig-more#sha384" => DigestId::Sha384,
        "http://www.w3.org/2001/04/xmlenc#sha512" => DigestId::Sha512,
        "http://www.w3.org/2000/09/xmldsig#sha1" => DigestId::Sha1,
        _ => return None,
    })
}

#[cfg(test)]
#[path = "verify_odf_tests.rs"]
mod tests;
