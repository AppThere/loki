// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Panic-freedom smoke tests: the decompressor and the project reader must
//! return `Result` (never panic) on malformed, truncated, or adversarial input.
//! In-tree complement to the `cargo-fuzz` targets (macro spec §12, T9).

use loki_vba::{VbaProject, decompress};

#[test]
fn decompress_never_panics_on_adversarial_input() {
    let cases: &[&[u8]] = &[
        &[],
        &[0x01],
        &[0x01, 0x00],
        &[0x01, 0xFF, 0xFF],       // claims 4096 data bytes it doesn't have
        &[0x01, 0xFF, 0xB0],       // compressed, huge declared size
        &[0x01, 0x00, 0xB0, 0xFF], // copy-heavy flag with no data
        &[0x01, 0x02, 0xB0, 0x01, 0x00, 0x00], // copy token at position 0
        &[0x01, 0xFF, 0x00],       // bad signature bits
    ];
    for c in cases {
        let _ = decompress(c);
    }
    // A long run of arbitrary bytes.
    let noise: Vec<u8> = (0u16..5000).map(|i| (i % 251) as u8).collect();
    let _ = decompress(&noise);
}

#[test]
fn decompress_bomb_guard_bounds_output() {
    // A single compressed chunk cannot expand past 4096 bytes; a crafted chunk
    // that tries to must error rather than allocate unboundedly.
    // flag byte 0xFF (8 copy tokens), each a max-length copy — but with no prior
    // output the first copy is invalid, so this must error, not loop.
    let mut input = vec![0x01u8, 0xFF, 0xB0]; // header claims a large chunk
    input.extend(std::iter::repeat_n(0xFFu8, 4096));
    let _ = decompress(&input); // must return (Ok or Err), never hang/panic
}

#[test]
fn read_never_panics() {
    let cases: &[&[u8]] = &[
        &[],
        b"not a compound file",
        &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1], // OLE magic, nothing else
    ];
    for c in cases {
        let _ = VbaProject::read(c);
    }
    let noise: Vec<u8> = (0u16..8192).map(|i| i as u8).collect();
    let _ = VbaProject::read(&noise);
}
