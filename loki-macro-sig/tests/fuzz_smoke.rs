// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Panic-freedom smoke tests: signature-stream extraction and the embedded-PKCS#7
//! locator must return (never panic) on malformed, truncated, or adversarial
//! input — parsing runs before any trust decision (macro spec §12, T9). In-tree
//! complement to the `cargo-fuzz` target.

use loki_macro_sig::extract_vba_signatures;

#[test]
fn extract_never_panics_on_adversarial_input() {
    let cases: &[&[u8]] = &[
        &[],
        &[0x00],
        b"not a compound file",
        &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1], // OLE magic, nothing else
        &[0x30, 0x80],                                     // SEQUENCE, indefinite length
        &[0x30, 0x84, 0xFF, 0xFF, 0xFF, 0xFF],             // SEQUENCE claiming ~4 GiB
        &[0x30, 0x0B, 0x06, 0x09, 0x2A, 0x86],             // truncated signedData OID
    ];
    for c in cases {
        let _ = extract_vba_signatures(c);
    }
    // A long run of arbitrary bytes, including stray SEQUENCE tags.
    let noise: Vec<u8> = (0u16..9000).map(|i| (i % 256) as u8).collect();
    let _ = extract_vba_signatures(&noise);
    let sequences: Vec<u8> = std::iter::repeat_n(0x30u8, 4000).collect();
    let _ = extract_vba_signatures(&sequences);
}
