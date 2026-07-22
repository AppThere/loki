// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the signature-summary view model and the `MacroService`
//! publisher-pinning surface (8A.7).

use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind, PreservedPart};
use loki_macro_sig::{CertInfo, SignatureVerdict, Thumbprint, UntrustedReason};

use super::{SignatureStatus, SignatureSummary};
use crate::service::MacroService;

fn payload(tag: &[u8]) -> MacroPayload {
    MacroPayload::new(
        MacroPayloadKind::OoxmlVba,
        vec![PreservedPart::new(
            "/word/vbaProject.bin",
            None,
            tag.to_vec(),
        )],
    )
}

fn cert(tp: u8) -> CertInfo {
    CertInfo {
        subject_cn: "Contoso Ltd".to_owned(),
        subject: "CN=Contoso Ltd".to_owned(),
        issuer: "CN=DigiCert".to_owned(),
        serial_hex: "01".to_owned(),
        not_before: 0,
        not_after: i64::MAX,
        thumbprint: Thumbprint::from_bytes([tp; 32]),
    }
}

fn untrusted(reason: UntrustedReason) -> SignatureVerdict {
    SignatureVerdict::ValidUntrusted {
        signer: cert(0xAB),
        reason,
    }
}

// ── view model ──────────────────────────────────────────────────────────────

#[test]
fn from_verdict_maps_every_state() {
    assert_eq!(
        SignatureSummary::from_verdict(&SignatureVerdict::Unsigned).status,
        SignatureStatus::Unsigned
    );
    assert_eq!(
        SignatureSummary::from_verdict(&SignatureVerdict::Invalid(
            loki_macro_sig::InvalidReason::DigestMismatch
        ))
        .status,
        SignatureStatus::Invalid
    );
    assert_eq!(
        SignatureSummary::from_verdict(&untrusted(UntrustedReason::NotPinned)).status,
        SignatureStatus::Untrusted
    );
    assert_eq!(
        SignatureSummary::from_verdict(&untrusted(UntrustedReason::LegacyAlgorithm)).status,
        SignatureStatus::Legacy
    );
    assert_eq!(
        SignatureSummary::from_verdict(&untrusted(UntrustedReason::CertificateExpired)).status,
        SignatureStatus::Expired
    );
    assert_eq!(
        SignatureSummary::from_verdict(&untrusted(UntrustedReason::PublisherRenewed)).status,
        SignatureStatus::Renewed
    );
}

#[test]
fn summary_carries_signer_fields_and_pin_affordance() {
    let s = SignatureSummary::from_verdict(&untrusted(UntrustedReason::NotPinned));
    assert_eq!(s.signer_cn.as_deref(), Some("Contoso Ltd"));
    assert_eq!(s.issuer.as_deref(), Some("CN=DigiCert"));
    assert_eq!(s.thumbprint_hex.unwrap().len(), 64);
    assert!(s.status.can_pin());
    assert!(
        !SignatureSummary::from_verdict(&untrusted(UntrustedReason::LegacyAlgorithm))
            .status
            .can_pin()
    );
    assert!(SignatureSummary::unsigned().signer_cn.is_none());
}

// ── service integration ─────────────────────────────────────────────────────

#[test]
fn no_recorded_signature_reads_unsigned() {
    let svc = MacroService::in_memory();
    assert_eq!(
        svc.signature_for(&payload(b"a")).status,
        SignatureStatus::Unsigned
    );
    assert!(!svc.is_publisher_trusted(&payload(b"a")));
}

#[test]
fn pinning_the_signer_flips_the_summary_to_trusted() {
    let svc = MacroService::in_memory();
    let p = payload(b"doc");
    svc.set_signature(&p, untrusted(UntrustedReason::NotPinned));

    assert_eq!(svc.signature_for(&p).status, SignatureStatus::Untrusted);
    assert!(!svc.is_publisher_trusted(&p));

    assert!(svc.pin_publisher(&p).expect("pin"));
    // The stored verdict now resolves against the pin — trusted, live.
    assert_eq!(svc.signature_for(&p).status, SignatureStatus::Trusted);
    assert!(svc.is_publisher_trusted(&p));
    assert_eq!(svc.trusted_publishers().len(), 1);
}

#[test]
fn pinning_an_unsigned_document_pins_nothing() {
    let svc = MacroService::in_memory();
    let p = payload(b"none");
    assert!(!svc.pin_publisher(&p).expect("pin"));
    assert!(svc.trusted_publishers().is_empty());
}

#[test]
fn unpinning_removes_trust_for_the_document() {
    let svc = MacroService::in_memory();
    let p = payload(b"doc");
    svc.set_signature(&p, untrusted(UntrustedReason::NotPinned));
    svc.pin_publisher(&p).expect("pin");
    let tp = svc.trusted_publishers()[0].thumbprint;

    svc.unpin_publisher(&tp).expect("unpin");
    assert!(svc.trusted_publishers().is_empty());
    assert_eq!(svc.signature_for(&p).status, SignatureStatus::Untrusted);
}

#[test]
fn expired_signer_does_not_become_trusted_when_pinned() {
    let svc = MacroService::in_memory();
    let p = payload(b"old");
    svc.set_signature(&p, untrusted(UntrustedReason::CertificateExpired));
    // Even after pinning, an expired signature stays Expired (never trusted).
    let _ = svc.pin_publisher(&p);
    assert_eq!(svc.signature_for(&p).status, SignatureStatus::Expired);
}
