// SPDX-License-Identifier: Apache-2.0

//! Tests for the Tier-2 X25519 DEK wrap.

use super::*;

#[test]
fn share_flow_round_trips() {
    // A sharing client wraps the DEK to a member's public key…
    let member_secret = X25519SecretKey::generate();
    let member_pk = member_secret.public_key();
    let dek = Dek::generate();
    let sharer = X25519KeyWrap::for_recipient(member_pk);
    let wrapped = sharer.wrap(&dek).unwrap();
    assert_eq!(wrapped.algorithm, X25519_WRAP_ALGORITHM);

    // …and only the member's client can unwrap it.
    let member = X25519KeyWrap::for_key_holder(member_secret);
    let unwrapped = member.unwrap_dek(&wrapped).unwrap();
    assert_eq!(unwrapped.as_bytes(), dek.as_bytes());
}

#[test]
fn server_side_wrap_only_instance_cannot_unwrap() {
    let member_secret = X25519SecretKey::generate();
    let sharer = X25519KeyWrap::for_recipient(member_secret.public_key());
    let wrapped = sharer.wrap(&Dek::generate()).unwrap();
    // The zero-knowledge property: without the secret key, unwrap fails.
    assert!(matches!(
        sharer.unwrap_dek(&wrapped),
        Err(CryptoError::DecryptFailed)
    ));
}

#[test]
fn wrong_member_cannot_unwrap() {
    let alice = X25519SecretKey::generate();
    let mallory = X25519SecretKey::generate();
    let wrapped = X25519KeyWrap::for_recipient(alice.public_key())
        .wrap(&Dek::generate())
        .unwrap();
    assert!(
        X25519KeyWrap::for_key_holder(mallory)
            .unwrap_dek(&wrapped)
            .is_err()
    );
}

#[test]
fn truncated_blob_is_typed_error() {
    let member_secret = X25519SecretKey::generate();
    let member = X25519KeyWrap::for_key_holder(member_secret);
    let wrapped = WrappedDek {
        algorithm: X25519_WRAP_ALGORITHM.to_owned(),
        blob: vec![0u8; 16],
    };
    assert!(matches!(
        member.unwrap_dek(&wrapped),
        Err(CryptoError::CiphertextTooShort(16))
    ));
}

#[test]
fn public_key_round_trips_through_bytes() {
    let secret = X25519SecretKey::generate();
    let pk = secret.public_key();
    let restored = X25519PublicKey::from_bytes(pk.as_bytes()).unwrap();
    assert_eq!(restored, pk);
    assert!(X25519PublicKey::from_bytes(&[0u8; 16]).is_err());
}
