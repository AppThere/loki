// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use std::io::{Cursor, Write};

use super::{RawVbaSignature, SigVariant, extract_vba_signatures};

/// A minimal DER `ContentInfo` whose type is PKCS#7 `signedData`:
/// `SEQUENCE (0x30 len 0x0B) { OID 1.2.840.113549.1.7.2 }`. Enough to exercise
/// the locator; a real one also carries the `[0]` `SignedData` content.
const SIGNED_DATA_CI: [u8; 13] = [
    0x30, 0x0B, 0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x07, 0x02,
];

const V3: &str = "\u{5}DigitalSignatureV3";
const AGILE: &str = "\u{5}DigitalSignatureAgile";
const LEGACY: &str = "\u{5}DigitalSignature";

/// Builds a `vbaProject.bin` carrying the named signature streams, each a junk
/// MS-OSHARED-wrapper prefix + the `signedData` `ContentInfo` + trailing junk.
fn signed_project(names: &[&str]) -> Vec<u8> {
    let mut comp = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
    comp.create_storage("/VBA").unwrap();
    for name in names {
        let mut content = vec![0xAAu8; 8]; // stand-in for the `DigSigInfoSerialized` header
        content.extend_from_slice(&SIGNED_DATA_CI);
        content.extend_from_slice(&[0xBBu8; 4]); // trailing cert-store bytes
        let mut s = comp.create_stream(format!("/{name}")).unwrap();
        s.write_all(&content).unwrap();
    }
    comp.flush().unwrap();
    comp.into_inner().into_inner()
}

#[test]
fn extracts_all_three_variants_strongest_first() {
    // Insertion order deliberately not strongest-first — extraction sorts.
    let bin = signed_project(&[LEGACY, V3, AGILE]);
    let sigs = extract_vba_signatures(&bin);
    let variants: Vec<_> = sigs.iter().map(|s| s.variant).collect();
    assert_eq!(
        variants,
        [SigVariant::V3, SigVariant::Agile, SigVariant::Legacy]
    );
}

#[test]
fn locates_pkcs7_amid_wrapper_bytes() {
    let bin = signed_project(&[V3]);
    let sigs = extract_vba_signatures(&bin);
    assert_eq!(sigs.len(), 1);
    // The PKCS#7 is found despite the 8-byte prefix and 4-byte suffix.
    assert_eq!(sigs[0].pkcs7_der(), Some(&SIGNED_DATA_CI[..]));
}

#[test]
fn legacy_is_not_trust_eligible_but_agile_and_v3_are() {
    assert!(!SigVariant::Legacy.is_trust_eligible());
    assert!(SigVariant::Agile.is_trust_eligible());
    assert!(SigVariant::V3.is_trust_eligible());
}

#[test]
fn long_form_der_length_is_located() {
    // SEQUENCE with a long-form length (0x81 0x82 = 130 content bytes): OID +
    // 119 filler. Exercises the multi-byte length path.
    let mut ci = vec![0x30, 0x81, 130];
    ci.extend_from_slice(&SIGNED_DATA_CI[2..]); // the OID (11 bytes)
    ci.extend(std::iter::repeat_n(0u8, 130 - 11)); // filler to reach the declared length
    let sig = RawVbaSignature {
        variant: SigVariant::V3,
        stream: ci.clone(),
    };
    assert_eq!(sig.pkcs7_der(), Some(&ci[..]));
}

#[test]
fn stream_without_signed_data_has_no_pkcs7() {
    let sig = RawVbaSignature {
        variant: SigVariant::Legacy,
        stream: vec![0x30, 0x03, 0x02, 0x01, 0x2A], // a SEQUENCE, but not signedData
    };
    assert!(sig.pkcs7_der().is_none());
}

#[test]
fn no_signature_streams_yields_empty() {
    let bin = signed_project(&[]); // has /VBA but no signature streams
    assert!(extract_vba_signatures(&bin).is_empty());
}

#[test]
fn non_container_input_is_empty() {
    assert!(extract_vba_signatures(b"not a compound file").is_empty());
    assert!(extract_vba_signatures(&[]).is_empty());
}
