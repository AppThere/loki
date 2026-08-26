// SPDX-License-Identifier: Apache-2.0

//! Known-answer vectors that pin the **persisted** crypto wire formats.
//!
//! Every other test in this crate is a symmetric round-trip: the same
//! implementation on both sides. That catches a one-sided change but is blind
//! to a *coordinated* one — swap the nonce prefix for a suffix, or the
//! algorithm-tag AAD for an empty one, and every round-trip still passes while
//! every already-stored blob becomes undecryptable. These formats are
//! persisted (`doc_meta.dek_wrapped`, `doc_member.dek_wrapped_for_user`,
//! sealed attachments in the object store) and, for Tier 2, decrypted by
//! independently-implemented clients — so the layout is a compatibility
//! contract, not an internal detail.
//!
//! # Provenance of the vectors (read before "fixing" a failure here)
//!
//! **The literals below were captured by running the current implementation
//! once on 2026-08-25 and pasting its output.** They are *not* independently
//! derived from RFC 8439 / RFC 5869 test vectors, and they do not prove the
//! primitives are correctly implemented — the underlying `chacha20poly1305`,
//! `hkdf` and `x25519-dalek` crates are trusted for that. What they do is
//! freeze *today's* framing so a later change to it cannot pass silently.
//!
//! Therefore: if an assertion in this file fails, the wire format changed.
//! That is a **breaking change** requiring a migration of every stored blob
//! and a matching update to every client implementation. Re-capturing the
//! vector to make the test green destroys the only instrument that reports it.

use std::collections::HashSet;

use crate::aead_wrap::AEAD_WRAP_ALGORITHM;
use crate::x25519_wrap::X25519_WRAP_ALGORITHM;
use crate::{AeadKeyWrap, Dek, Kek, KeyWrap, WrappedDek};

/// Decodes a hex vector literal. Panics on malformed input: the inputs are
/// literals in this file, so malformation is a test-authoring bug.
pub(crate) fn unhex(s: &str) -> Vec<u8> {
    assert!(s.len().is_multiple_of(2), "hex literal has odd length");
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}

/// XChaCha20-Poly1305 nonce length, restated here so the layout assertions
/// below fail if `dek.rs` changes its own constant.
const NONCE_LEN: usize = 24;
/// Poly1305 tag length.
const TAG_LEN: usize = 16;

/// Fixed DEK: bytes 0x00..=0x1f.
const KAT_KEY_HEX: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
/// `Dek::seal(KAT_PLAINTEXT, KAT_AAD)` under `KAT_KEY_HEX`, captured from the
/// current implementation on 2026-08-25. Layout: `nonce(24) || ct || tag(16)`.
const KAT_SEALED_HEX: &str = concat!(
    "44091407925acc101bcb99daf33a216edc5e88e70fb846ff",
    "3bc54eef6a3ba29182a746be5b9d6e7b7272e20e344a2ff8",
    "d971a94a7c13f0548d976851727ddd8e",
);
const KAT_PLAINTEXT: &[u8] = b"loki known-answer vector";
const KAT_AAD: &[u8] = b"doc-kat";

/// Fixed KEK: bytes 0x40..=0x5f.
const KAT_KEK_HEX: &str = "404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f";
/// The DEK that gets wrapped in the vectors: bytes 0x80..=0x9f.
const KAT_WRAPPED_DEK_HEX: &str =
    "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f";
/// `AeadKeyWrap::new(KAT_KEK).wrap(KAT_WRAPPED_DEK).blob`, captured from the
/// current implementation on 2026-08-25.
const KAT_AEAD_BLOB_HEX: &str = concat!(
    "5c4d668111c2e94f6d137205b6e10a7cca175599ff981ca2",
    "7f2c2ee966070ab3b4c03234847edbed77c1fb3bfb75470e",
    "26b9023fe3a7cfdd17fc573efd2f44a36d2b9ab0c06d98fa",
);

// ---------------------------------------------------------------------------
// dek.rs — `nonce(24) || ciphertext+tag` (F-CR-1)
// ---------------------------------------------------------------------------

#[test]
fn open_decrypts_a_pinned_sealed_blob() {
    let dek = Dek::from_bytes(&unhex(KAT_KEY_HEX)).unwrap();
    let sealed = unhex(KAT_SEALED_HEX);
    assert_eq!(dek.open(&sealed, KAT_AAD).unwrap(), KAT_PLAINTEXT);
}

#[test]
fn pinned_sealed_blob_has_the_documented_layout() {
    let sealed = unhex(KAT_SEALED_HEX);
    // nonce || ciphertext (stream cipher: same length as the plaintext) || tag
    assert_eq!(sealed.len(), NONCE_LEN + KAT_PLAINTEXT.len() + TAG_LEN);

    let dek = Dek::from_bytes(&unhex(KAT_KEY_HEX)).unwrap();
    // Inversion: if the leading 24 bytes were not consumed as the nonce,
    // corrupting them would not break authentication.
    for i in [0usize, NONCE_LEN - 1] {
        let mut tampered = sealed.clone();
        tampered[i] ^= 0x01;
        assert!(
            dek.open(&tampered, KAT_AAD).is_err(),
            "byte {i} is not being consumed as nonce material"
        );
    }
    // And the trailing 16 bytes really are the tag.
    let mut tampered = sealed.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 0x01;
    assert!(dek.open(&tampered, KAT_AAD).is_err());
}

// ---------------------------------------------------------------------------
// dek.rs — nonce freshness (F-CR-3)
// ---------------------------------------------------------------------------

#[test]
fn seal_draws_a_fresh_nonce_on_every_call() {
    // XChaCha20-Poly1305 nonce reuse under one key leaks the keystream and
    // forfeits authentication. No other test in this crate seals twice with
    // the same key, so an RNG regression returning a constant nonce would pass
    // the whole suite.
    let dek = Dek::from_bytes(&unhex(KAT_KEY_HEX)).unwrap();
    let first = dek.seal(KAT_PLAINTEXT, KAT_AAD).unwrap();
    let second = dek.seal(KAT_PLAINTEXT, KAT_AAD).unwrap();
    assert_ne!(
        first[..NONCE_LEN],
        second[..NONCE_LEN],
        "nonce reuse: two seals under one key drew the same nonce"
    );
    assert_ne!(first, second, "identical ciphertext for identical input");
    // Both must still open — a "different" nonce that broke decryption would
    // satisfy the assertions above without being correct.
    assert_eq!(dek.open(&first, KAT_AAD).unwrap(), KAT_PLAINTEXT);
    assert_eq!(dek.open(&second, KAT_AAD).unwrap(), KAT_PLAINTEXT);

    // A wider sample also catches a low-entropy or small-counter regression
    // that happens to differ between two adjacent calls.
    let nonces: HashSet<Vec<u8>> = (0..32)
        .map(|_| dek.seal(KAT_PLAINTEXT, KAT_AAD).unwrap()[..NONCE_LEN].to_vec())
        .collect();
    assert_eq!(nonces.len(), 32, "repeated nonce across 32 seals");
}

// ---------------------------------------------------------------------------
// aead_wrap.rs — the Tier-0/1 wrapped-DEK format stored in `doc_meta`
// ---------------------------------------------------------------------------

#[test]
fn aead_wrap_unwraps_a_pinned_blob() {
    let wrapper = AeadKeyWrap::new(Kek::from_bytes(&unhex(KAT_KEK_HEX)).unwrap());
    let wrapped = WrappedDek {
        algorithm: AEAD_WRAP_ALGORITHM.to_owned(),
        blob: unhex(KAT_AEAD_BLOB_HEX),
    };
    let dek = wrapper.unwrap_dek(&wrapped).unwrap();
    assert_eq!(dek.as_bytes().as_slice(), unhex(KAT_WRAPPED_DEK_HEX));
}

#[test]
fn aead_wrap_blob_is_a_sealed_dek_bound_to_the_algorithm_tag() {
    // Pins two things `unwrap_dek` alone cannot distinguish: that the blob is
    // exactly a `Dek::seal` output under the KEK (no extra framing), and that
    // the AAD is the algorithm tag rather than, say, empty.
    let kek_as_cipher = Dek::from_bytes(&unhex(KAT_KEK_HEX)).unwrap();
    let blob = unhex(KAT_AEAD_BLOB_HEX);
    assert_eq!(blob.len(), NONCE_LEN + crate::DEK_LEN + TAG_LEN);
    assert_eq!(
        kek_as_cipher
            .open(&blob, AEAD_WRAP_ALGORITHM.as_bytes())
            .unwrap(),
        unhex(KAT_WRAPPED_DEK_HEX)
    );
    assert!(
        kek_as_cipher.open(&blob, b"").is_err(),
        "AAD is not the algorithm tag"
    );
}

#[test]
fn algorithm_tags_are_stable_strings() {
    // These strings are persisted in `doc_meta.dek_wrapped` /
    // `doc_member.dek_wrapped_for_user` and dispatched on at unwrap time
    // (crypto-agility, ADR-C014): renaming one orphans stored rows.
    assert_eq!(AEAD_WRAP_ALGORITHM, "xchacha20-poly1305-kek.v1");
    assert_eq!(X25519_WRAP_ALGORITHM, "x25519-hkdf-sha256-xchacha20.v1");
}

// ---------------------------------------------------------------------------
// wrap.rs — the JSON/base64 serialization contract (F-CR-2)
// ---------------------------------------------------------------------------

/// `serde_json::to_string` of a `WrappedDek` with blob `0x00..=0x0f`,
/// captured from the current implementation on 2026-08-25.
const KAT_JSON: &str =
    r#"{"algorithm":"xchacha20-poly1305-kek.v1","blob":"AAECAwQFBgcICQoLDA0ODw=="}"#;

#[test]
fn wrapped_dek_serializes_to_the_pinned_json() {
    let wrapped = WrappedDek {
        algorithm: AEAD_WRAP_ALGORITHM.to_owned(),
        blob: (0..16u8).collect(),
    };
    assert_eq!(serde_json::to_string(&wrapped).unwrap(), KAT_JSON);
    let back: WrappedDek = serde_json::from_str(KAT_JSON).unwrap();
    assert_eq!(back, wrapped);
}

#[test]
fn wrapped_dek_base64_uses_the_standard_alphabet_with_padding() {
    // A round-trip cannot tell STANDARD from URL_SAFE or NO_PAD; only the
    // characters can. Chosen bytes encode to `++++////` plus a padded group.
    let wrapped = WrappedDek {
        algorithm: "t".to_owned(),
        blob: vec![0xfb, 0xef, 0xbe, 0xff, 0xff, 0xff, 0x00],
    };
    let json = serde_json::to_string(&wrapped).unwrap();
    assert!(json.contains('+'), "URL-safe alphabet ('-') in use: {json}");
    assert!(json.contains('/'), "URL-safe alphabet ('_') in use: {json}");
    assert!(json.contains("=="), "padding dropped (NO_PAD): {json}");
}
