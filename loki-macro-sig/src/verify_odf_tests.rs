// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! End-to-end tests for the ODF `XMLDSig` verifier. Each test builds a real
//! `macrosignatures.xml`: the `SignedInfo` is hand-authored in **canonical** form
//! (so the signed octets do not depend on our own canonicaliser), signed with a
//! fresh self-signed RSA or P-256 key, and embedded verbatim. Real `LibreOffice`
//! interop is the `TODO(8A.4-corpus)` gate.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use pkcs8::{EncodePrivateKey, LineEnding};
use rsa::RsaPrivateKey;
use rsa::pkcs1v15::SigningKey;
use rsa::signature::{SignatureEncoding, Signer};
use sha1::Sha1;
use sha2::{Digest, Sha256};

use crate::verdict::{InvalidReason, SignatureVerdict, UntrustedReason};
use crate::verify_xmldsig;

const MODULE: &[u8] = b"Sub AutoOpen()\r\n  MsgBox \"hi\"\r\nEnd Sub\r\n";
const MODULE_URI: &str = "Basic/Standard/Module1.xml";
const C14N: &str = "http://www.w3.org/TR/2001/REC-xml-c14n-20010315";
const SHA256_URI: &str = "http://www.w3.org/2001/04/xmlenc#sha256";
const RSA_SHA256: &str = "http://www.w3.org/2001/04/xmldsig-more#rsa-sha256";
const RSA_SHA1: &str = "http://www.w3.org/2000/09/xmldsig#rsa-sha1";
const ECDSA_SHA256: &str = "http://www.w3.org/2001/04/xmldsig-more#ecdsa-sha256";

// --- fixture assembly -------------------------------------------------------

fn b64(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

/// A canonical `SignedInfo` with one package-part reference (SHA-256 digest of
/// `part`), for the given signature method.
fn signed_info(part: &[u8], sig_method: &str) -> String {
    let digest = b64(&Sha256::digest(part));
    format!(
        "<SignedInfo xmlns=\"http://www.w3.org/2000/09/xmldsig#\">\
<CanonicalizationMethod Algorithm=\"{C14N}\"></CanonicalizationMethod>\
<SignatureMethod Algorithm=\"{sig_method}\"></SignatureMethod>\
<Reference URI=\"{MODULE_URI}\">\
<DigestMethod Algorithm=\"{SHA256_URI}\"></DigestMethod>\
<DigestValue>{digest}</DigestValue>\
</Reference></SignedInfo>"
    )
}

/// Wraps a `SignedInfo`, signature value, and certificate into a full document.
fn macrosig(signed_info: &str, sig_value: &[u8], cert_der: &[u8]) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<document-signatures xmlns=\"urn:oasis:names:tc:opendocument:xmlns:digitalsignature:1.0\">\
<Signature xmlns=\"http://www.w3.org/2000/09/xmldsig#\" Id=\"ID_1\">\
{signed_info}\
<SignatureValue>{}</SignatureValue>\
<KeyInfo><X509Data><X509Certificate>{}</X509Certificate></X509Data></KeyInfo>\
</Signature></document-signatures>",
        b64(sig_value),
        b64(cert_der)
    )
}

fn resolve_module(uri: &str) -> Option<Vec<u8>> {
    (uri == MODULE_URI).then(|| MODULE.to_vec())
}

fn cert_from_key_pair(kp: &rcgen::KeyPair, cn: &str, expired: bool) -> Vec<u8> {
    let mut params = rcgen::CertificateParams::new(Vec::new()).unwrap();
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, cn);
    if expired {
        params.not_before = rcgen::date_time_ymd(2000, 1, 1);
        params.not_after = rcgen::date_time_ymd(2001, 1, 1);
    } else {
        params.not_before = rcgen::date_time_ymd(2020, 1, 1);
        params.not_after = rcgen::date_time_ymd(2035, 1, 1);
    }
    params.self_signed(kp).unwrap().der().to_vec()
}

fn rsa_key() -> RsaPrivateKey {
    let mut rng = rand::thread_rng();
    RsaPrivateKey::new(&mut rng, 2048).unwrap()
}

fn rsa_cert(key: &RsaPrivateKey, cn: &str, expired: bool) -> Vec<u8> {
    let pem = key.to_pkcs8_pem(LineEnding::LF).unwrap();
    let kp = rcgen::KeyPair::from_pkcs8_pem_and_sign_algo(&pem, &rcgen::PKCS_RSA_SHA256).unwrap();
    cert_from_key_pair(&kp, cn, expired)
}

// --- happy paths ------------------------------------------------------------

#[test]
fn rsa_sha256_valid_is_untrusted_not_pinned() {
    let key = rsa_key();
    let cert = rsa_cert(&key, "Contoso Ltd", false);
    let si = signed_info(MODULE, RSA_SHA256);
    let sig = SigningKey::<Sha256>::new(key).sign(si.as_bytes()).to_vec();
    let xml = macrosig(&si, &sig, &cert);

    match verify_xmldsig(xml.as_bytes(), resolve_module) {
        SignatureVerdict::ValidUntrusted { signer, reason } => {
            assert_eq!(reason, UntrustedReason::NotPinned);
            assert_eq!(signer.subject_cn, "Contoso Ltd");
        }
        other => panic!("expected ValidUntrusted/NotPinned, got {other:?}"),
    }
}

#[test]
fn ecdsa_p256_sha256_valid_is_untrusted() {
    let mut rng = rand::thread_rng();
    let secret = p256::SecretKey::random(&mut rng);
    let pem = secret.to_pkcs8_pem(LineEnding::LF).unwrap();
    let kp =
        rcgen::KeyPair::from_pkcs8_pem_and_sign_algo(&pem, &rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
    let cert = cert_from_key_pair(&kp, "Fabrikam Inc", false);

    let si = signed_info(MODULE, ECDSA_SHA256);
    let signing_key = p256::ecdsa::SigningKey::from(&secret);
    let sig: p256::ecdsa::Signature = signing_key.sign(si.as_bytes());
    let xml = macrosig(&si, &sig.to_bytes(), &cert);

    match verify_xmldsig(xml.as_bytes(), resolve_module) {
        SignatureVerdict::ValidUntrusted { reason, .. } => {
            assert_eq!(reason, UntrustedReason::NotPinned);
        }
        other => panic!("expected ValidUntrusted, got {other:?}"),
    }
}

// --- failure paths ----------------------------------------------------------

#[test]
fn tampered_part_is_content_mismatch() {
    let key = rsa_key();
    let cert = rsa_cert(&key, "Contoso Ltd", false);
    let si = signed_info(MODULE, RSA_SHA256);
    let sig = SigningKey::<Sha256>::new(key).sign(si.as_bytes()).to_vec();
    let xml = macrosig(&si, &sig, &cert);

    // Resolver returns different bytes than were signed.
    let verdict = verify_xmldsig(xml.as_bytes(), |uri| {
        (uri == MODULE_URI).then(|| b"tampered module source".to_vec())
    });
    assert_eq!(
        verdict,
        SignatureVerdict::Invalid(InvalidReason::ContentMismatch)
    );
}

#[test]
fn missing_part_is_content_mismatch() {
    let key = rsa_key();
    let cert = rsa_cert(&key, "Contoso Ltd", false);
    let si = signed_info(MODULE, RSA_SHA256);
    let sig = SigningKey::<Sha256>::new(key).sign(si.as_bytes()).to_vec();
    let xml = macrosig(&si, &sig, &cert);

    let verdict = verify_xmldsig(xml.as_bytes(), |_| None);
    assert_eq!(
        verdict,
        SignatureVerdict::Invalid(InvalidReason::ContentMismatch)
    );
}

#[test]
fn corrupt_signature_is_digest_mismatch() {
    let key = rsa_key();
    let cert = rsa_cert(&key, "Contoso Ltd", false);
    let si = signed_info(MODULE, RSA_SHA256);
    let mut sig = SigningKey::<Sha256>::new(key).sign(si.as_bytes()).to_vec();
    let n = sig.len();
    sig[n - 1] ^= 0xFF;
    let xml = macrosig(&si, &sig, &cert);

    let verdict = verify_xmldsig(xml.as_bytes(), resolve_module);
    assert_eq!(
        verdict,
        SignatureVerdict::Invalid(InvalidReason::DigestMismatch)
    );
}

#[test]
fn expired_cert_is_certificate_expired() {
    let key = rsa_key();
    let cert = rsa_cert(&key, "Old Publisher", true);
    let si = signed_info(MODULE, RSA_SHA256);
    let sig = SigningKey::<Sha256>::new(key).sign(si.as_bytes()).to_vec();
    let xml = macrosig(&si, &sig, &cert);

    match verify_xmldsig(xml.as_bytes(), resolve_module) {
        SignatureVerdict::ValidUntrusted { reason, .. } => {
            assert_eq!(reason, UntrustedReason::CertificateExpired);
        }
        other => panic!("expected CertificateExpired, got {other:?}"),
    }
}

#[test]
fn legacy_rsa_sha1_is_legacy_algorithm() {
    let key = rsa_key();
    let cert = rsa_cert(&key, "Legacy Publisher", false);
    let si = signed_info(MODULE, RSA_SHA1);
    let sig = SigningKey::<Sha1>::new(key).sign(si.as_bytes()).to_vec();
    let xml = macrosig(&si, &sig, &cert);

    match verify_xmldsig(xml.as_bytes(), resolve_module) {
        SignatureVerdict::ValidUntrusted { reason, .. } => {
            assert_eq!(reason, UntrustedReason::LegacyAlgorithm);
        }
        other => panic!("expected LegacyAlgorithm, got {other:?}"),
    }
}

#[test]
fn unknown_signature_method_is_unsupported() {
    let key = rsa_key();
    let cert = rsa_cert(&key, "Contoso Ltd", false);
    let si = signed_info(MODULE, "urn:example:not-a-real-alg");
    let sig = SigningKey::<Sha256>::new(key).sign(si.as_bytes()).to_vec();
    let xml = macrosig(&si, &sig, &cert);

    assert_eq!(
        verify_xmldsig(xml.as_bytes(), resolve_module),
        SignatureVerdict::Invalid(InvalidReason::UnsupportedAlgorithm)
    );
}

#[test]
fn no_signature_is_unsigned() {
    assert_eq!(
        verify_xmldsig(b"not xml at all", resolve_module),
        SignatureVerdict::Unsigned
    );
    let empty = "<?xml version=\"1.0\"?><document-signatures xmlns=\"urn:oasis:names:tc:opendocument:xmlns:digitalsignature:1.0\"></document-signatures>";
    assert_eq!(
        verify_xmldsig(empty.as_bytes(), resolve_module),
        SignatureVerdict::Unsigned
    );
}

#[test]
fn adversarial_input_never_panics() {
    let cases: &[&[u8]] = &[
        b"",
        b"<",
        b"<Signature",
        b"<a><b></a>",
        b"<document-signatures><Signature><SignedInfo>",
        &[0xFF, 0xFE, 0x00, 0x01],
    ];
    for c in cases {
        let _ = verify_xmldsig(c, resolve_module);
    }
}
