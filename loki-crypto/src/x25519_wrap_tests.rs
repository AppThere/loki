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

// ---------------------------------------------------------------------------
// Known-answer vectors for the Tier-2 wire format (F-CR-1).
//
// Tier-2 unwrapping happens on clients that may be implemented independently
// (see the module doc on `x25519_wrap.rs`), so the blob layout and the HKDF
// construction are an interop contract. The round-trips above run the same
// code in both directions and cannot see a coordinated change to either.
//
// PROVENANCE: the literals below were captured by running the current
// implementation once on 2026-08-25 — they are not independent RFC 7748 /
// RFC 5869 vectors and prove nothing about the primitives themselves. They
// freeze today's framing. A failure here means the wire format changed: a
// breaking change requiring migration of every stored wrapped DEK and a
// matching client update, never a signal to re-capture the vector.
// See `vectors_tests.rs` for the same discipline applied to `dek.rs`.
// ---------------------------------------------------------------------------

use crate::vectors_tests::unhex;

/// Fixed ephemeral secret: bytes 0x10..=0x2f.
const KAT_EPHEMERAL_SK_HEX: &str =
    "101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f";
/// The X25519 public key derived from `KAT_EPHEMERAL_SK_HEX`.
const KAT_EPHEMERAL_PK_HEX: &str =
    "d89e3bad79437dbed9f843418304f460ff05c7fe81fe4a9577a804cb9367ff66";
/// Fixed recipient secret: bytes 0x60..=0x7f.
const KAT_RECIPIENT_SK_HEX: &str =
    "606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f";
/// The X25519 public key derived from `KAT_RECIPIENT_SK_HEX`.
const KAT_RECIPIENT_PK_HEX: &str =
    "675dd574ed7789310b3d2e7681f3790b466c773b1521fecf36577958371ea52f";
/// The X25519 shared secret for the two keys above.
const KAT_SHARED_HEX: &str = "87176d73e808875e2b02fde12202e466e1ce13b30d8ff117b2542b71162bed0f";
/// `derive_wrapping_key(shared, ephemeral_pk, recipient_pk)` — this is the
/// value that pins the HKDF salt (`ephemeral_pk || recipient_pk`), its order,
/// and the `info` string (the algorithm tag).
const KAT_WRAPPING_KEY_HEX: &str =
    "44a69fcbbc8880693b9377c6552eb7a05a732debefc937ddabfc5aebd50ccf85";
/// The DEK carried by `KAT_BLOB_HEX`: bytes 0x80..=0x9f.
const KAT_PAYLOAD_DEK_HEX: &str =
    "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f";
/// A wrap of `KAT_PAYLOAD_DEK_HEX` to `KAT_RECIPIENT_PK_HEX` (with a random
/// ephemeral key, hence unrelated to `KAT_EPHEMERAL_SK_HEX`).
/// Layout: `ephemeral_pk(32) || nonce(24) || ciphertext(32) || tag(16)`.
const KAT_BLOB_HEX: &str = concat!(
    "a4e453c034baa87c76d9281a964653198e6928aa9993e09e",
    "b2f3838de0b6d077f2ceb9060c8e6bc371d75587a4b9201b",
    "375101744fe389fa4d038f559a521bbd4da22244e46b89c1",
    "7e14bdab3f6b6aaeba26d7ad95dc5730429afd506b24d069",
    "5cf4138ee4fe91da",
);

/// Rebuilds the pinned keypairs and their shared secret.
fn kat_keys() -> (StaticSecret, PublicKey, X25519PublicKey) {
    let sk: [u8; 32] = unhex(KAT_EPHEMERAL_SK_HEX).try_into().unwrap();
    let ephemeral = StaticSecret::from(sk);
    let ephemeral_pk = PublicKey::from(&ephemeral);
    let recipient_pk = X25519SecretKey::from_bytes(&unhex(KAT_RECIPIENT_SK_HEX))
        .unwrap()
        .public_key();
    (ephemeral, ephemeral_pk, recipient_pk)
}

#[test]
fn hkdf_wrapping_key_matches_the_pinned_vector() {
    let (ephemeral, ephemeral_pk, recipient_pk) = kat_keys();
    assert_eq!(
        ephemeral_pk.as_bytes().as_slice(),
        unhex(KAT_EPHEMERAL_PK_HEX)
    );
    assert_eq!(
        recipient_pk.as_bytes().as_slice(),
        unhex(KAT_RECIPIENT_PK_HEX)
    );

    let shared = ephemeral.diffie_hellman(&recipient_pk.0);
    assert_eq!(shared.as_bytes().as_slice(), unhex(KAT_SHARED_HEX));

    let wrapping_key =
        X25519KeyWrap::derive_wrapping_key(&shared, &ephemeral_pk, &recipient_pk).unwrap();
    assert_eq!(
        wrapping_key.as_bytes().as_slice(),
        unhex(KAT_WRAPPING_KEY_HEX),
        "HKDF salt/info construction changed — stored Tier-2 DEKs can no longer be unwrapped"
    );
}

#[test]
fn hkdf_salt_is_order_sensitive() {
    // The salt is documented as `ephemeral_pk || recipient_pk`. If the two
    // halves were concatenated in the other order — or if the salt were
    // ignored — swapping them would produce the same key, and the pinned
    // vector above would not actually be pinning the order.
    let (ephemeral, ephemeral_pk, recipient_pk) = kat_keys();
    let shared = ephemeral.diffie_hellman(&recipient_pk.0);
    let forward =
        X25519KeyWrap::derive_wrapping_key(&shared, &ephemeral_pk, &recipient_pk).unwrap();
    let swapped = X25519KeyWrap::derive_wrapping_key(
        &shared,
        &recipient_pk.0,
        &X25519PublicKey(ephemeral_pk),
    )
    .unwrap();
    assert_ne!(forward.as_bytes(), swapped.as_bytes());
}

#[test]
fn unwrap_dek_opens_a_pinned_blob() {
    let member = X25519SecretKey::from_bytes(&unhex(KAT_RECIPIENT_SK_HEX)).unwrap();
    let wrapped = WrappedDek {
        algorithm: X25519_WRAP_ALGORITHM.to_owned(),
        blob: unhex(KAT_BLOB_HEX),
    };
    let dek = X25519KeyWrap::for_key_holder(member)
        .unwrap_dek(&wrapped)
        .unwrap();
    assert_eq!(dek.as_bytes().as_slice(), unhex(KAT_PAYLOAD_DEK_HEX));
}

#[test]
fn pinned_blob_has_the_documented_layout() {
    let blob = unhex(KAT_BLOB_HEX);
    // ephemeral_pk(32) || nonce(24) || ciphertext(32) || tag(16)
    assert_eq!(blob.len(), 32 + 24 + DEK_LEN + 16);

    let wrapped = |b: Vec<u8>| WrappedDek {
        algorithm: X25519_WRAP_ALGORITHM.to_owned(),
        blob: b,
    };
    let member = || {
        X25519KeyWrap::for_key_holder(
            X25519SecretKey::from_bytes(&unhex(KAT_RECIPIENT_SK_HEX)).unwrap(),
        )
    };
    // Inversion: the leading 32 bytes must be *consumed as the ephemeral
    // public key* (they feed the DH and the HKDF salt), not skipped padding.
    for i in [0usize, 31] {
        let mut tampered = blob.clone();
        tampered[i] ^= 0x01;
        assert!(
            member().unwrap_dek(&wrapped(tampered)).is_err(),
            "byte {i} is not being consumed as ephemeral-public-key material"
        );
    }
    // And the bytes after it are the sealed DEK, prefixed by its own nonce.
    let mut tampered = blob.clone();
    tampered[32] ^= 0x01;
    assert!(member().unwrap_dek(&wrapped(tampered)).is_err());
}
