// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Open-path verification tests: a preserved [`MacroPayload`] is verified through
//! the ODF XMLDSig path end-to-end, and the trusted-publisher enable-at-open gate
//! is exercised through [`MacroService`].

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind, PreservedPart};
use loki_macro_sig::SignatureVerdict;
use pkcs8::{EncodePrivateKey, LineEnding};
use rsa::RsaPrivateKey;
use rsa::pkcs1v15::SigningKey;
use rsa::signature::{SignatureEncoding, Signer};
use sha2::{Digest, Sha256};

use crate::service::{MacroService, SignatureStatus};
use crate::verify::verify_payload;

const MODULE_URI: &str = "Basic/Standard/Module1.xml";
const MODULE: &[u8] = b"<module>Sub AutoOpen()\nEnd Sub</module>";

fn b64(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

/// A canonical `SignedInfo` covering the module part.
fn signed_info() -> String {
    let digest = b64(&Sha256::digest(MODULE));
    format!(
        "<SignedInfo xmlns=\"http://www.w3.org/2000/09/xmldsig#\">\
<CanonicalizationMethod Algorithm=\"http://www.w3.org/TR/2001/REC-xml-c14n-20010315\"></CanonicalizationMethod>\
<SignatureMethod Algorithm=\"http://www.w3.org/2001/04/xmldsig-more#rsa-sha256\"></SignatureMethod>\
<Reference URI=\"{MODULE_URI}\">\
<DigestMethod Algorithm=\"http://www.w3.org/2001/04/xmlenc#sha256\"></DigestMethod>\
<DigestValue>{digest}</DigestValue>\
</Reference></SignedInfo>"
    )
}

/// Builds a signed `macrosignatures.xml` and returns it plus the signer cert DER.
fn macrosignatures() -> (Vec<u8>, RsaPrivateKey) {
    let mut rng = rand::thread_rng();
    let key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
    let pem = key.to_pkcs8_pem(LineEnding::LF).unwrap();
    let kp = rcgen::KeyPair::from_pkcs8_pem_and_sign_algo(&pem, &rcgen::PKCS_RSA_SHA256).unwrap();
    let mut params = rcgen::CertificateParams::new(Vec::new()).unwrap();
    params.distinguished_name = rcgen::DistinguishedName::new();
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "Contoso Ltd");
    params.not_before = rcgen::date_time_ymd(2020, 1, 1);
    params.not_after = rcgen::date_time_ymd(2035, 1, 1);
    let cert = params.self_signed(&kp).unwrap().der().to_vec();

    let si = signed_info();
    let sig = SigningKey::<Sha256>::new(key.clone())
        .sign(si.as_bytes())
        .to_vec();
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<document-signatures xmlns=\"urn:oasis:names:tc:opendocument:xmlns:digitalsignature:1.0\">\
<Signature xmlns=\"http://www.w3.org/2000/09/xmldsig#\" Id=\"ID_1\">\
{si}\
<SignatureValue>{}</SignatureValue>\
<KeyInfo><X509Data><X509Certificate>{}</X509Certificate></X509Data></KeyInfo>\
</Signature></document-signatures>",
        b64(&sig),
        b64(&cert),
    );
    (xml.into_bytes(), key)
}

fn odf_payload(sig_xml: Option<Vec<u8>>, module: &[u8]) -> MacroPayload {
    let mut parts = vec![PreservedPart::new(
        MODULE_URI,
        Some("text/xml".to_owned()),
        module.to_vec(),
    )];
    if let Some(xml) = sig_xml {
        parts.push(PreservedPart::new(
            "META-INF/macrosignatures.xml",
            Some("text/xml".to_owned()),
            xml,
        ));
    }
    MacroPayload::new(MacroPayloadKind::OdfBasic, parts)
}

// ── verify_payload ──────────────────────────────────────────────────────────

#[test]
fn signed_odf_payload_verifies_as_valid_untrusted() {
    let (sig, _key) = macrosignatures();
    let payload = odf_payload(Some(sig), MODULE);
    match verify_payload(&payload) {
        SignatureVerdict::ValidUntrusted { signer, .. } => {
            assert_eq!(signer.subject_cn, "Contoso Ltd");
        }
        other => panic!("expected ValidUntrusted, got {other:?}"),
    }
}

#[test]
fn tampered_module_fails_content_mismatch() {
    let (sig, _key) = macrosignatures();
    // The preserved module bytes differ from what was signed.
    let payload = odf_payload(Some(sig), b"<module>evil</module>");
    assert!(matches!(
        verify_payload(&payload),
        SignatureVerdict::Invalid(_)
    ));
}

#[test]
fn unsigned_odf_payload_is_unsigned() {
    let payload = odf_payload(None, MODULE);
    assert_eq!(verify_payload(&payload), SignatureVerdict::Unsigned);
}

#[test]
fn vba_payload_is_unsigned_pending_content_hash() {
    // VBA verification is deferred (MS-OVBA content hash) — never a false Invalid.
    let payload = MacroPayload::new(
        MacroPayloadKind::OoxmlVba,
        vec![PreservedPart::new(
            "/word/vbaProject.bin",
            None,
            vec![1, 2, 3],
        )],
    );
    assert_eq!(verify_payload(&payload), SignatureVerdict::Unsigned);
}

// ── enable-at-open through MacroService ─────────────────────────────────────

#[test]
fn trusted_publisher_enables_at_open() {
    let (sig, _key) = macrosignatures();
    let payload = odf_payload(Some(sig), MODULE);
    let svc = MacroService::in_memory();

    // Before verification, nothing is enabled.
    assert!(!svc.is_enabled(&payload));

    // On open: verify + record. The signer is not yet pinned → untrusted, so the
    // document is not enabled at open (needs the per-document click).
    let summary = svc.verify_and_record(&payload);
    assert_eq!(summary.status, SignatureStatus::Untrusted);
    assert!(!svc.is_enabled(&payload));

    // Pin the publisher → now enabled at open, no per-document trust record.
    assert!(svc.pin_publisher(&payload).expect("pin"));
    assert!(svc.is_publisher_trusted(&payload));
    assert!(svc.is_enabled(&payload));
    assert_eq!(svc.signature_for(&payload).status, SignatureStatus::Trusted);
}
