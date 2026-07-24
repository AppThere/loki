// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for [`TrustedPublisherStore`]: pinning, the verdict→trust upgrade, the
//! renewal path, persistence, and the downgrade/expiry defences.

use loki_macro_sig::{CertInfo, SignatureVerdict, Thumbprint, UntrustedReason};

use super::{PublisherRecord, TrustedPublisherStore};

fn cert(tp: u8, subject: &str, issuer: &str) -> CertInfo {
    CertInfo {
        subject_cn: "Contoso Ltd".to_owned(),
        subject: subject.to_owned(),
        issuer: issuer.to_owned(),
        serial_hex: "01".to_owned(),
        not_before: 0,
        not_after: i64::MAX,
        thumbprint: Thumbprint::from_bytes([tp; 32]),
    }
}

fn untrusted(info: CertInfo) -> SignatureVerdict {
    SignatureVerdict::ValidUntrusted {
        signer: info,
        reason: UntrustedReason::NotPinned,
    }
}

#[test]
fn pinned_signer_upgrades_to_valid_trusted() {
    let info = cert(0xAB, "CN=Contoso", "CN=DigiCert");
    let mut store = TrustedPublisherStore::new(None);
    store.pin(PublisherRecord::from_cert_info(&info));

    match store.resolve(untrusted(info.clone())) {
        SignatureVerdict::ValidTrusted { thumbprint, signer } => {
            assert_eq!(thumbprint, info.thumbprint);
            assert_eq!(signer.thumbprint, info.thumbprint);
        }
        other => panic!("expected ValidTrusted, got {other:?}"),
    }
}

#[test]
fn unpinned_signer_stays_untrusted() {
    let info = cert(0x01, "CN=Contoso", "CN=DigiCert");
    let store = TrustedPublisherStore::new(None);
    assert_eq!(store.resolve(untrusted(info.clone())), untrusted(info));
}

#[test]
fn renewed_certificate_becomes_publisher_renewed() {
    // Pin the old cert; a new cert with the same identity but a new thumbprint.
    let old = cert(0x11, "CN=Contoso", "CN=DigiCert");
    let new = cert(0x22, "CN=Contoso", "CN=DigiCert");
    let mut store = TrustedPublisherStore::new(None);
    store.pin(PublisherRecord::from_cert_info(&old));

    match store.resolve(untrusted(new.clone())) {
        SignatureVerdict::ValidUntrusted { reason, signer } => {
            assert_eq!(reason, UntrustedReason::PublisherRenewed);
            assert_eq!(signer.thumbprint, new.thumbprint);
        }
        other => panic!("expected PublisherRenewed, got {other:?}"),
    }
}

#[test]
fn different_identity_is_not_a_renewal() {
    let pinned = cert(0x11, "CN=Contoso", "CN=DigiCert");
    let other = cert(0x22, "CN=Evil Corp", "CN=DigiCert");
    let mut store = TrustedPublisherStore::new(None);
    store.pin(PublisherRecord::from_cert_info(&pinned));
    // Same issuer, different subject → plain NotPinned, no renewal affordance.
    assert_eq!(store.resolve(untrusted(other.clone())), untrusted(other));
}

#[test]
fn legacy_and_expired_never_upgrade_even_when_pinned() {
    let info = cert(0xAB, "CN=Contoso", "CN=DigiCert");
    let mut store = TrustedPublisherStore::new(None);
    store.pin(PublisherRecord::from_cert_info(&info));

    for reason in [
        UntrustedReason::LegacyAlgorithm,
        UntrustedReason::CertificateExpired,
    ] {
        let verdict = SignatureVerdict::ValidUntrusted {
            signer: info.clone(),
            reason,
        };
        assert_eq!(
            store.resolve(verdict.clone()),
            verdict,
            "{reason:?} must stay untrusted despite the pin"
        );
    }
}

#[test]
fn invalid_and_unsigned_pass_through_unchanged() {
    let store = TrustedPublisherStore::new(None);
    assert_eq!(
        store.resolve(SignatureVerdict::Unsigned),
        SignatureVerdict::Unsigned
    );
    let invalid = SignatureVerdict::Invalid(loki_macro_sig::InvalidReason::DigestMismatch);
    assert_eq!(store.resolve(invalid.clone()), invalid);
}

#[test]
fn pin_contains_and_unpin() {
    let info = cert(0x42, "CN=A", "CN=B");
    let tp = *info.thumbprint.as_bytes();
    let mut store = TrustedPublisherStore::new(None);
    assert!(store.is_empty());
    store.pin(PublisherRecord::from_cert_info(&info));
    assert!(store.contains(&tp));
    assert_eq!(store.len(), 1);
    let removed = store.unpin(&tp).expect("was pinned");
    assert_eq!(removed.thumbprint, tp);
    assert!(!store.contains(&tp));
    assert!(store.is_empty());
}

#[test]
fn from_cert_info_uses_cn_and_records_identity() {
    let info = cert(0x07, "CN=Contoso, O=Contoso Ltd", "CN=DigiCert");
    let rec = PublisherRecord::from_cert_info(&info);
    assert_eq!(rec.display_name, "Contoso Ltd"); // subject_cn
    assert_eq!(rec.subject, "CN=Contoso, O=Contoso Ltd");
    assert_eq!(rec.issuer, "CN=DigiCert");
    assert_eq!(rec.thumbprint, *info.thumbprint.as_bytes());
}

#[test]
fn persists_and_reloads() {
    let dir = std::env::temp_dir().join(format!("loki-pub-{}", std::process::id()));
    let path = dir.join("publishers.json");
    let _ = std::fs::remove_file(&path);

    let info = cert(0x99, "CN=Contoso", "CN=DigiCert");
    let tp = *info.thumbprint.as_bytes();
    {
        let mut store = TrustedPublisherStore::new(Some(path.clone()));
        store.pin(PublisherRecord::from_cert_info(&info));
        store.save().expect("save");
    }
    let reloaded = TrustedPublisherStore::load(path.clone()).expect("load");
    assert!(reloaded.contains(&tp));
    assert_eq!(
        reloaded.records().next().unwrap().display_name,
        "Contoso Ltd"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn corrupt_file_degrades_to_empty() {
    let dir = std::env::temp_dir().join(format!("loki-pub-corrupt-{}", std::process::id()));
    let path = dir.join("publishers.json");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(&path, b"{ not json").expect("write");
    assert!(TrustedPublisherStore::load(path.clone()).is_err());
    assert!(TrustedPublisherStore::load_or_empty(path.clone()).is_empty());
    let _ = std::fs::remove_file(&path);
}
