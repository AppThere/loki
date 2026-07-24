// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use super::{CertInfo, InvalidReason, SignatureVerdict, Thumbprint, UntrustedReason};

fn cert(thumb: [u8; 32]) -> CertInfo {
    CertInfo {
        subject_cn: "Contoso Ltd".into(),
        subject: "CN=Contoso Ltd, O=Contoso".into(),
        issuer: "CN=Example CA".into(),
        serial_hex: "0a1b2c".into(),
        not_before: 0,
        not_after: i64::MAX,
        thumbprint: Thumbprint::from_bytes(thumb),
    }
}

#[test]
fn thumbprint_hex_round_trips_the_bytes() {
    let mut bytes = [0u8; 32];
    bytes[0] = 0xDE;
    bytes[1] = 0xAD;
    bytes[31] = 0x0F;
    let t = Thumbprint::from_bytes(bytes);
    assert_eq!(&t.to_hex()[..4], "dead");
    assert!(t.to_hex().ends_with("0f"));
    assert_eq!(t.to_hex().len(), 64);
    assert_eq!(t.as_bytes(), &bytes);
}

#[test]
fn thumbprint_equality_is_by_value() {
    assert_eq!(
        Thumbprint::from_bytes([7; 32]),
        Thumbprint::from_bytes([7; 32])
    );
    assert_ne!(
        Thumbprint::from_bytes([7; 32]),
        Thumbprint::from_bytes([8; 32])
    );
}

#[test]
fn only_valid_trusted_is_trusted() {
    let thumb = [1u8; 32];
    let trusted = SignatureVerdict::ValidTrusted {
        signer: cert(thumb),
        thumbprint: Thumbprint::from_bytes(thumb),
    };
    assert!(trusted.is_trusted());
    assert!(trusted.is_signed());

    // A valid-but-unpinned signature is signed, but NOT trusted (T10).
    let untrusted = SignatureVerdict::ValidUntrusted {
        signer: cert(thumb),
        reason: UntrustedReason::NotPinned,
    };
    assert!(
        !untrusted.is_trusted(),
        "a bare valid signature must never be trusted"
    );
    assert!(untrusted.is_signed());

    for v in [
        SignatureVerdict::Unsigned,
        SignatureVerdict::Invalid(InvalidReason::DigestMismatch),
    ] {
        assert!(!v.is_trusted());
    }
}

#[test]
fn is_signed_and_signer_track_the_variant() {
    assert!(!SignatureVerdict::Unsigned.is_signed());
    assert!(SignatureVerdict::Invalid(InvalidReason::Malformed).is_signed());

    let thumb = [2u8; 32];
    let v = SignatureVerdict::ValidUntrusted {
        signer: cert(thumb),
        reason: UntrustedReason::CertificateExpired,
    };
    assert_eq!(
        v.signer().map(|c| c.subject_cn.as_str()),
        Some("Contoso Ltd")
    );

    // Unsigned / Invalid carry no signer.
    assert!(SignatureVerdict::Unsigned.signer().is_none());
    assert!(
        SignatureVerdict::Invalid(InvalidReason::ContentMismatch)
            .signer()
            .is_none()
    );
}
