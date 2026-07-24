// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Integration tests for the CMS `SignedData` verifier. Each test generates a
//! *real* detached PKCS#7 over `content` — a fresh self-signed cert (RSA or
//! P-256) plus signed attributes carrying the content digest — then asserts the
//! verdict. No fixtures on disk: everything is built in-process so the crypto is
//! exercised end to end.

use cms::cert::{CertificateChoices, IssuerAndSerialNumber};
use cms::content_info::{CmsVersion, ContentInfo};
use cms::signed_data::{
    CertificateSet, DigestAlgorithmIdentifiers, EncapsulatedContentInfo, SignedData,
    SignerIdentifier, SignerInfo, SignerInfos,
};
use const_oid::ObjectIdentifier;
use const_oid::db::rfc5911::{ID_CONTENT_TYPE, ID_DATA, ID_MESSAGE_DIGEST, ID_SIGNED_DATA};
use const_oid::db::rfc5912::{ECDSA_WITH_SHA_256, ID_SHA_1, ID_SHA_256, RSA_ENCRYPTION};
use der::asn1::{OctetString, SetOfVec};
use der::{Any, Decode, Encode, Tag};
use pkcs8::{EncodePrivateKey, LineEnding};
use rsa::RsaPrivateKey;
use rsa::pkcs1v15::SigningKey;
use rsa::signature::{SignatureEncoding, Signer};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use spki::AlgorithmIdentifierOwned;
use x509_cert::Certificate;
use x509_cert::attr::Attribute;

use crate::verdict::{InvalidReason, SignatureVerdict, Thumbprint, UntrustedReason};
use crate::verify::verify_signed_data;

const CONTENT: &[u8] = b"Attribute VBA_Signature\r\nSub AutoOpen()\r\nEnd Sub\r\n";

// --- fixture assembly -------------------------------------------------------

/// A signature over `signed_attrs` DER, returning the raw signature octets.
type SignFn<'a> = dyn Fn(&[u8]) -> Vec<u8> + 'a;

/// Assembles a detached CMS `SignedData` `ContentInfo` (DER) for `cert_der`,
/// with signed attributes (`content-type` + `message-digest` = `content_digest`)
/// signed by `sign`.
fn build_pkcs7(
    cert_der: &[u8],
    digest_oid: ObjectIdentifier,
    sig_oid: ObjectIdentifier,
    content_digest: &[u8],
    sign: &SignFn,
) -> Vec<u8> {
    let cert = Certificate::from_der(cert_der).unwrap();
    let alg = |oid| AlgorithmIdentifierOwned {
        oid,
        parameters: None,
    };

    let ct_value = Any::new(Tag::ObjectIdentifier, ID_DATA.as_bytes()).unwrap();
    let ct_attr = Attribute {
        oid: ID_CONTENT_TYPE,
        values: SetOfVec::try_from(vec![ct_value]).unwrap(),
    };
    let md_value = Any::new(Tag::OctetString, content_digest).unwrap();
    let md_attr = Attribute {
        oid: ID_MESSAGE_DIGEST,
        values: SetOfVec::try_from(vec![md_value]).unwrap(),
    };
    let signed_attrs = SetOfVec::try_from(vec![ct_attr, md_attr]).unwrap();
    let signed_attrs_der = signed_attrs.to_der().unwrap();
    let signature = sign(&signed_attrs_der);

    let signer_info = SignerInfo {
        version: CmsVersion::V1,
        sid: SignerIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
            issuer: cert.tbs_certificate.issuer.clone(),
            serial_number: cert.tbs_certificate.serial_number.clone(),
        }),
        digest_alg: alg(digest_oid),
        signed_attrs: Some(signed_attrs),
        signature_algorithm: alg(sig_oid),
        signature: OctetString::new(signature).unwrap(),
        unsigned_attrs: None,
    };

    let signed_data = SignedData {
        version: CmsVersion::V1,
        digest_algorithms: DigestAlgorithmIdentifiers::try_from(vec![alg(digest_oid)]).unwrap(),
        encap_content_info: EncapsulatedContentInfo {
            econtent_type: ID_DATA,
            econtent: None,
        },
        certificates: Some(CertificateSet(
            SetOfVec::try_from(vec![CertificateChoices::Certificate(cert)]).unwrap(),
        )),
        crls: None,
        signer_infos: SignerInfos(SetOfVec::try_from(vec![signer_info]).unwrap()),
    };

    let sd_der = signed_data.to_der().unwrap();
    let content_info = ContentInfo {
        content_type: ID_SIGNED_DATA,
        content: Any::from_der(&sd_der).unwrap(),
    };
    content_info.to_der().unwrap()
}

/// Builds a self-signed cert around an rcgen `KeyPair`, with `cn` and either a
/// current or an already-past validity window.
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

fn expected_thumbprint(cert_der: &[u8]) -> Thumbprint {
    let digest: [u8; 32] = Sha256::digest(cert_der).into();
    Thumbprint::from_bytes(digest)
}

// --- happy paths ------------------------------------------------------------

#[test]
fn rsa_sha256_valid_is_untrusted_not_pinned() {
    let key = rsa_key();
    let cert_der = rsa_cert(&key, "Contoso Ltd", false);
    let signer = SigningKey::<Sha256>::new(key.clone());
    let sign = move |msg: &[u8]| signer.sign(msg).to_vec();
    let digest = Sha256::digest(CONTENT).to_vec();
    let p7 = build_pkcs7(&cert_der, ID_SHA_256, RSA_ENCRYPTION, &digest, &sign);

    match verify_signed_data(&p7, CONTENT) {
        SignatureVerdict::ValidUntrusted { signer, reason } => {
            assert_eq!(reason, UntrustedReason::NotPinned);
            assert_eq!(signer.subject_cn, "Contoso Ltd");
            assert!(signer.subject.contains("Contoso Ltd"));
            assert_eq!(signer.thumbprint, expected_thumbprint(&cert_der));
            assert!(!signer.serial_hex.is_empty());
        }
        other => panic!("expected ValidUntrusted/NotPinned, got {other:?}"),
    }
}

#[test]
fn ecdsa_p256_sha256_valid_is_untrusted_not_pinned() {
    let mut rng = rand::thread_rng();
    let secret = p256::SecretKey::random(&mut rng);
    let pem = secret.to_pkcs8_pem(LineEnding::LF).unwrap();
    let kp =
        rcgen::KeyPair::from_pkcs8_pem_and_sign_algo(&pem, &rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
    let cert_der = cert_from_key_pair(&kp, "Fabrikam Inc", false);

    let signing_key = p256::ecdsa::SigningKey::from(&secret);
    let sign = move |msg: &[u8]| {
        use p256::ecdsa::DerSignature;
        let sig: DerSignature = signing_key.sign(msg);
        sig.as_bytes().to_vec()
    };
    let digest = Sha256::digest(CONTENT).to_vec();
    let p7 = build_pkcs7(&cert_der, ID_SHA_256, ECDSA_WITH_SHA_256, &digest, &sign);

    match verify_signed_data(&p7, CONTENT) {
        SignatureVerdict::ValidUntrusted { signer, reason } => {
            assert_eq!(reason, UntrustedReason::NotPinned);
            assert_eq!(signer.subject_cn, "Fabrikam Inc");
            assert_eq!(signer.thumbprint, expected_thumbprint(&cert_der));
        }
        other => panic!("expected ValidUntrusted/NotPinned, got {other:?}"),
    }
}

// --- failure paths ----------------------------------------------------------

#[test]
fn tampered_content_is_content_mismatch() {
    let key = rsa_key();
    let cert_der = rsa_cert(&key, "Contoso Ltd", false);
    let signer = SigningKey::<Sha256>::new(key.clone());
    let sign = move |msg: &[u8]| signer.sign(msg).to_vec();
    let digest = Sha256::digest(CONTENT).to_vec();
    let p7 = build_pkcs7(&cert_der, ID_SHA_256, RSA_ENCRYPTION, &digest, &sign);

    let verdict = verify_signed_data(&p7, b"a completely different macro body");
    assert_eq!(
        verdict,
        SignatureVerdict::Invalid(InvalidReason::ContentMismatch)
    );
}

#[test]
fn broken_signature_is_digest_mismatch() {
    let key = rsa_key();
    let cert_der = rsa_cert(&key, "Contoso Ltd", false);
    let signer = SigningKey::<Sha256>::new(key.clone());
    // message-digest still matches content, but the signature octets are corrupt.
    let sign = move |msg: &[u8]| {
        let mut s = signer.sign(msg).to_vec();
        let n = s.len();
        s[n - 1] ^= 0xFF;
        s
    };
    let digest = Sha256::digest(CONTENT).to_vec();
    let p7 = build_pkcs7(&cert_der, ID_SHA_256, RSA_ENCRYPTION, &digest, &sign);

    let verdict = verify_signed_data(&p7, CONTENT);
    assert_eq!(
        verdict,
        SignatureVerdict::Invalid(InvalidReason::DigestMismatch)
    );
}

#[test]
fn expired_cert_is_certificate_expired() {
    let key = rsa_key();
    let cert_der = rsa_cert(&key, "Old Publisher", true);
    let signer = SigningKey::<Sha256>::new(key.clone());
    let sign = move |msg: &[u8]| signer.sign(msg).to_vec();
    let digest = Sha256::digest(CONTENT).to_vec();
    let p7 = build_pkcs7(&cert_der, ID_SHA_256, RSA_ENCRYPTION, &digest, &sign);

    match verify_signed_data(&p7, CONTENT) {
        SignatureVerdict::ValidUntrusted { reason, .. } => {
            assert_eq!(reason, UntrustedReason::CertificateExpired);
        }
        other => panic!("expected ValidUntrusted/CertificateExpired, got {other:?}"),
    }
}

#[test]
fn legacy_sha1_digest_is_legacy_algorithm() {
    let key = rsa_key();
    let cert_der = rsa_cert(&key, "Legacy Publisher", false);
    let signer = SigningKey::<Sha1>::new(key.clone());
    let sign = move |msg: &[u8]| signer.sign(msg).to_vec();
    let digest = Sha1::digest(CONTENT).to_vec();
    let p7 = build_pkcs7(&cert_der, ID_SHA_1, RSA_ENCRYPTION, &digest, &sign);

    match verify_signed_data(&p7, CONTENT) {
        SignatureVerdict::ValidUntrusted { reason, .. } => {
            assert_eq!(reason, UntrustedReason::LegacyAlgorithm);
        }
        other => panic!("expected ValidUntrusted/LegacyAlgorithm, got {other:?}"),
    }
}

// --- garbage / panic-freedom ------------------------------------------------

#[test]
fn garbage_input_is_malformed() {
    assert_eq!(
        verify_signed_data(b"not der", b"x"),
        SignatureVerdict::Invalid(InvalidReason::Malformed)
    );
}

#[test]
fn adversarial_bytes_never_panic() {
    let cases: &[&[u8]] = &[
        &[],
        &[0x00],
        &[0x30],
        &[0x30, 0x80],
        &[0x30, 0x84, 0xFF, 0xFF, 0xFF, 0xFF],
        &[
            0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x07, 0x02,
        ],
        b"\x30\x0b\x06\x09\x2a\x86\x48\x86\xf7\x0d\x01\x07\x02",
    ];
    for c in cases {
        let _ = verify_signed_data(c, CONTENT);
        let _ = verify_signed_data(c, &[]);
    }
    let noise: Vec<u8> = (0u16..6000).map(|i| (i % 251) as u8).collect();
    let _ = verify_signed_data(&noise, CONTENT);
}
