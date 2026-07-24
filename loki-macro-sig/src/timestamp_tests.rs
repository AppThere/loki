// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! End-to-end tests for RFC-3161 timestamp handling. Each builds a real CMS
//! `SignedData` over `content` signed by an **expired** self-signed cert, then
//! attaches an `id-aa-timeStampToken` unsigned attribute whose `messageImprint`
//! binds to the outer signature. A `genTime` inside the cert validity rescues the
//! expiry (`NotPinned`); anything else leaves it `CertificateExpired`.

use std::time::Duration;

use cms::cert::{CertificateChoices, IssuerAndSerialNumber};
use cms::content_info::{CmsVersion, ContentInfo};
use cms::signed_data::{
    CertificateSet, DigestAlgorithmIdentifiers, EncapsulatedContentInfo, SignedData,
    SignerIdentifier, SignerInfo, SignerInfos,
};
use const_oid::ObjectIdentifier;
use const_oid::db::rfc5911::{ID_CONTENT_TYPE, ID_DATA, ID_MESSAGE_DIGEST, ID_SIGNED_DATA};
use const_oid::db::rfc5912::{ID_SHA_256, RSA_ENCRYPTION};
use der::asn1::{GeneralizedTime, OctetString, SetOfVec};
use der::{Any, Decode, Encode, Sequence, Tag};
use pkcs8::{EncodePrivateKey, LineEnding};
use rsa::RsaPrivateKey;
use rsa::pkcs1v15::SigningKey;
use rsa::signature::{SignatureEncoding, Signer};
use sha2::{Digest, Sha256};
use spki::AlgorithmIdentifierOwned;
use x509_cert::Certificate;
use x509_cert::attr::Attribute;

use crate::verdict::{SignatureVerdict, UntrustedReason};
use crate::verify::verify_signed_data;

const CONTENT: &[u8] = b"Attribute VBA_Signature\r\nSub AutoOpen()\r\nEnd Sub\r\n";
const ID_AA_TIMESTAMP_TOKEN: ObjectIdentifier =
    ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.16.2.14");
const ID_CT_TSTINFO: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.16.1.4");
// The expired fixture cert is valid [2000-01-01, 2001-01-01); these bracket it.
const IN_WINDOW_SECS: u64 = 960_000_000; // ~2000-06
const OUT_OF_WINDOW_SECS: u64 = 1_262_304_000; // 2010-01-01

#[derive(Sequence)]
struct MessageImprintBuild {
    hash_algorithm: AlgorithmIdentifierOwned,
    hashed_message: OctetString,
}

#[derive(Sequence)]
struct TstInfoBuild {
    version: u8,
    policy: ObjectIdentifier,
    message_imprint: MessageImprintBuild,
    serial_number: u8,
    gen_time: GeneralizedTime,
}

fn alg(oid: ObjectIdentifier) -> AlgorithmIdentifierOwned {
    AlgorithmIdentifierOwned {
        oid,
        parameters: None,
    }
}

/// A `TimeStampToken` `Any` whose `messageImprint` is `SHA-256(sig_octets)` and
/// whose `genTime` is `gen_secs`.
fn timestamp_token(sig_octets: &[u8], gen_secs: u64) -> Any {
    let imprint = Sha256::digest(sig_octets).to_vec();
    let tst = TstInfoBuild {
        version: 1,
        policy: ObjectIdentifier::new_unwrap("1.2.3.4.5"),
        message_imprint: MessageImprintBuild {
            hash_algorithm: alg(ID_SHA_256),
            hashed_message: OctetString::new(imprint).unwrap(),
        },
        serial_number: 1,
        gen_time: GeneralizedTime::from_unix_duration(Duration::from_secs(gen_secs)).unwrap(),
    };
    let tst_der = tst.to_der().unwrap();

    let signed_data = SignedData {
        version: CmsVersion::V3,
        digest_algorithms: DigestAlgorithmIdentifiers::try_from(vec![]).unwrap(),
        encap_content_info: EncapsulatedContentInfo {
            econtent_type: ID_CT_TSTINFO,
            econtent: Some(Any::new(Tag::OctetString, tst_der).unwrap()),
        },
        certificates: None,
        crls: None,
        signer_infos: SignerInfos(SetOfVec::new()),
    };
    let sd_der = signed_data.to_der().unwrap();
    let content_info = ContentInfo {
        content_type: ID_SIGNED_DATA,
        content: Any::from_der(&sd_der).unwrap(),
    };
    Any::from_der(&content_info.to_der().unwrap()).unwrap()
}

/// Builds a detached CMS over `CONTENT` signed by `cert_der`'s key, optionally
/// attaching `timestamp` as the `id-aa-timeStampToken` unsigned attribute.
fn build_pkcs7(cert_der: &[u8], key: &RsaPrivateKey, timestamp: Timestamp) -> Vec<u8> {
    let cert = Certificate::from_der(cert_der).unwrap();

    let ct_value = Any::new(Tag::ObjectIdentifier, ID_DATA.as_bytes()).unwrap();
    let ct_attr = Attribute {
        oid: ID_CONTENT_TYPE,
        values: SetOfVec::try_from(vec![ct_value]).unwrap(),
    };
    let content_digest = Sha256::digest(CONTENT).to_vec();
    let md_value = Any::new(Tag::OctetString, content_digest).unwrap();
    let md_attr = Attribute {
        oid: ID_MESSAGE_DIGEST,
        values: SetOfVec::try_from(vec![md_value]).unwrap(),
    };
    let signed_attrs = SetOfVec::try_from(vec![ct_attr, md_attr]).unwrap();

    let signer = SigningKey::<Sha256>::new(key.clone());
    let signature = signer.sign(&signed_attrs.to_der().unwrap()).to_vec();

    let unsigned_attrs = match timestamp {
        Timestamp::None => None,
        Timestamp::At(secs) => {
            let token = timestamp_token(&signature, secs);
            let attr = Attribute {
                oid: ID_AA_TIMESTAMP_TOKEN,
                values: SetOfVec::try_from(vec![token]).unwrap(),
            };
            Some(SetOfVec::try_from(vec![attr]).unwrap())
        }
        Timestamp::UnboundAt(secs) => {
            // A token whose imprint is over the wrong bytes → not bound.
            let token = timestamp_token(b"a different signature", secs);
            let attr = Attribute {
                oid: ID_AA_TIMESTAMP_TOKEN,
                values: SetOfVec::try_from(vec![token]).unwrap(),
            };
            Some(SetOfVec::try_from(vec![attr]).unwrap())
        }
    };

    let signer_info = SignerInfo {
        version: CmsVersion::V1,
        sid: SignerIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
            issuer: cert.tbs_certificate.issuer.clone(),
            serial_number: cert.tbs_certificate.serial_number.clone(),
        }),
        digest_alg: alg(ID_SHA_256),
        signed_attrs: Some(signed_attrs),
        signature_algorithm: alg(RSA_ENCRYPTION),
        signature: OctetString::new(signature).unwrap(),
        unsigned_attrs,
    };

    let signed_data = SignedData {
        version: CmsVersion::V1,
        digest_algorithms: DigestAlgorithmIdentifiers::try_from(vec![alg(ID_SHA_256)]).unwrap(),
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

#[derive(Clone, Copy)]
enum Timestamp {
    None,
    At(u64),
    UnboundAt(u64),
}

fn expired_key_and_cert() -> (RsaPrivateKey, Vec<u8>) {
    let mut rng = rand::thread_rng();
    let key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
    let pem = key.to_pkcs8_pem(LineEnding::LF).unwrap();
    let kp = rcgen::KeyPair::from_pkcs8_pem_and_sign_algo(&pem, &rcgen::PKCS_RSA_SHA256).unwrap();
    let mut params = rcgen::CertificateParams::new(Vec::new()).unwrap();
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "Old Publisher");
    params.not_before = rcgen::date_time_ymd(2000, 1, 1);
    params.not_after = rcgen::date_time_ymd(2001, 1, 1);
    let cert = params.self_signed(&kp).unwrap().der().to_vec();
    (key, cert)
}

fn reason(verdict: SignatureVerdict) -> UntrustedReason {
    match verdict {
        SignatureVerdict::ValidUntrusted { reason, .. } => reason,
        other => panic!("expected ValidUntrusted, got {other:?}"),
    }
}

#[test]
fn timestamp_within_validity_rescues_expired_cert() {
    let (key, cert) = expired_key_and_cert();
    let p7 = build_pkcs7(&cert, &key, Timestamp::At(IN_WINDOW_SECS));
    assert_eq!(
        reason(verify_signed_data(&p7, CONTENT)),
        UntrustedReason::NotPinned,
        "a timestamp proving signing-while-valid must clear the expiry"
    );
}

#[test]
fn expired_cert_without_timestamp_stays_expired() {
    let (key, cert) = expired_key_and_cert();
    let p7 = build_pkcs7(&cert, &key, Timestamp::None);
    assert_eq!(
        reason(verify_signed_data(&p7, CONTENT)),
        UntrustedReason::CertificateExpired
    );
}

#[test]
fn timestamp_outside_validity_does_not_rescue() {
    let (key, cert) = expired_key_and_cert();
    let p7 = build_pkcs7(&cert, &key, Timestamp::At(OUT_OF_WINDOW_SECS));
    assert_eq!(
        reason(verify_signed_data(&p7, CONTENT)),
        UntrustedReason::CertificateExpired
    );
}

#[test]
fn unbound_timestamp_is_ignored() {
    // A timestamp whose messageImprint is over other bytes must not rescue —
    // otherwise a timestamp could be transplanted from another signature.
    let (key, cert) = expired_key_and_cert();
    let p7 = build_pkcs7(&cert, &key, Timestamp::UnboundAt(IN_WINDOW_SECS));
    assert_eq!(
        reason(verify_signed_data(&p7, CONTENT)),
        UntrustedReason::CertificateExpired
    );
}
