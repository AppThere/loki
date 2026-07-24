// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Panic-freedom smoke tests: signature-stream extraction and the embedded-PKCS#7
//! locator must return (never panic) on malformed, truncated, or adversarial
//! input — parsing runs before any trust decision (macro spec §12, T9). In-tree
//! complement to the `cargo-fuzz` target.

use loki_macro_sig::{extract_vba_signatures, verify_signed_data, verify_xmldsig};

const ADVERSARIAL: &[&[u8]] = &[
    &[],
    &[0x00],
    b"not a compound file",
    &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1], // OLE magic, nothing else
    &[0x30, 0x80],                                     // SEQUENCE, indefinite length
    &[0x30, 0x84, 0xFF, 0xFF, 0xFF, 0xFF],             // SEQUENCE claiming ~4 GiB
    &[0x30, 0x0B, 0x06, 0x09, 0x2A, 0x86],             // truncated signedData OID
    &[
        0x30, 0x0B, 0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x07, 0x02,
    ], // bare signedData ContentInfo, no body
];

#[test]
fn extract_never_panics_on_adversarial_input() {
    for c in ADVERSARIAL {
        let _ = extract_vba_signatures(c);
    }
    // A long run of arbitrary bytes, including stray SEQUENCE tags.
    let noise: Vec<u8> = (0u16..9000).map(|i| (i % 256) as u8).collect();
    let _ = extract_vba_signatures(&noise);
    let sequences: Vec<u8> = std::iter::repeat_n(0x30u8, 4000).collect();
    let _ = extract_vba_signatures(&sequences);
}

#[test]
fn verify_never_panics_on_adversarial_input() {
    // The CMS verifier runs before any trust decision (T9): it must degrade to
    // `Invalid`, never panic, on malformed, truncated, or random DER.
    for c in ADVERSARIAL {
        let _ = verify_signed_data(c, b"content");
        let _ = verify_signed_data(c, &[]);
    }
    let noise: Vec<u8> = (0u16..12000).map(|i| (i % 253) as u8).collect();
    let _ = verify_signed_data(&noise, b"content");
    // Truncations of a plausible DER prefix.
    let prefix: &[u8] = &[0x30, 0x82, 0x03, 0x21, 0x06, 0x09, 0x2A, 0x86, 0x48];
    for n in 0..prefix.len() {
        let _ = verify_signed_data(&prefix[..n], b"x");
    }
}

#[test]
fn verify_xmldsig_never_panics_on_adversarial_input() {
    // The ODF XMLDSig verifier parses, canonicalises, and verifies before any
    // trust decision (T9): malformed XML/base64/DER must degrade to a verdict.
    let resolve = |uri: &str| Some(uri.as_bytes().to_vec());
    for c in ADVERSARIAL {
        let _ = verify_xmldsig(c, &[], resolve);
    }
    let partials: &[&[u8]] = &[
        b"<document-signatures><Signature><SignedInfo>",
        b"<Signature xmlns=\"http://www.w3.org/2000/09/xmldsig#\"><SignedInfo/></Signature>",
        b"<a xmlns=\"urn:x\"><b xmlns:p=\"\"></b></a>",
        b"<r>&notanentity; &amp; &#x41;</r>",
    ];
    for c in partials {
        let _ = verify_xmldsig(c, &[], resolve);
    }
    let noise: Vec<u8> = (0u16..12000).map(|i| (i % 251) as u8).collect();
    let _ = verify_xmldsig(&noise, &[], resolve);
}
