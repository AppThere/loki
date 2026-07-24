// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the algorithm-agility mapping — pure, no fixtures.

use const_oid::ObjectIdentifier;
use const_oid::db::rfc5912::{
    ECDSA_WITH_SHA_256, ID_MD_5, ID_SHA_1, ID_SHA_256, ID_SHA_384, ID_SHA_512, RSA_ENCRYPTION,
    SHA_256_WITH_RSA_ENCRYPTION,
};

use super::{DigestId, SigKind};

#[test]
fn digest_oid_maps_and_flags_legacy() {
    assert_eq!(DigestId::from_oid(&ID_SHA_256), Some(DigestId::Sha256));
    assert_eq!(DigestId::from_oid(&ID_SHA_384), Some(DigestId::Sha384));
    assert_eq!(DigestId::from_oid(&ID_SHA_512), Some(DigestId::Sha512));
    assert_eq!(DigestId::from_oid(&ID_SHA_1), Some(DigestId::Sha1));
    assert_eq!(DigestId::from_oid(&ID_MD_5), Some(DigestId::Md5));

    assert!(!DigestId::Sha256.is_legacy());
    assert!(!DigestId::Sha384.is_legacy());
    assert!(!DigestId::Sha512.is_legacy());
    assert!(DigestId::Sha1.is_legacy());
    assert!(DigestId::Md5.is_legacy());

    let unknown = ObjectIdentifier::new_unwrap("1.2.3.4.5");
    assert_eq!(DigestId::from_oid(&unknown), None);
}

#[test]
fn signature_oid_maps_families() {
    assert_eq!(SigKind::from_oid(&RSA_ENCRYPTION), Some(SigKind::Rsa));
    assert_eq!(
        SigKind::from_oid(&SHA_256_WITH_RSA_ENCRYPTION),
        Some(SigKind::Rsa)
    );
    assert_eq!(SigKind::from_oid(&ECDSA_WITH_SHA_256), Some(SigKind::Ecdsa));

    let unknown = ObjectIdentifier::new_unwrap("1.2.3.4.5");
    assert_eq!(SigKind::from_oid(&unknown), None);
}

#[test]
fn digest_produces_expected_lengths() {
    assert_eq!(DigestId::Sha256.digest(b"loki").len(), 32);
    assert_eq!(DigestId::Sha384.digest(b"loki").len(), 48);
    assert_eq!(DigestId::Sha512.digest(b"loki").len(), 64);
    assert_eq!(DigestId::Sha1.digest(b"loki").len(), 20);
    assert_eq!(DigestId::Md5.digest(b"loki").len(), 16);
}
