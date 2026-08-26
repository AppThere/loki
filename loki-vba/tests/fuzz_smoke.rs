// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Panic-freedom smoke tests: the decompressor and the project reader must
//! return `Result` (never panic) on malformed, truncated, or adversarial input.
//! In-tree complement to the `cargo-fuzz` targets (macro spec §12, T9).

use loki_vba::{VbaError, VbaProject, compress, decompress};

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
    // A crafted chunk that tries to expand past the 4096-byte per-chunk cap must
    // error rather than allocate unboundedly. The window is seeded with a real
    // literal first, so the copy token that follows is *valid* and expansion
    // actually happens — otherwise the container fails on the copy offset and
    // the bomb guard is never reached (which is what this test used to do).
    //
    // [flag 0x02, 'A', copy(offset=1, length=4098)] → 4099 bytes in one chunk.
    let mut input = vec![0x01u8, 0x03, 0xB0];
    input.extend_from_slice(&[0x02, b'A', 0xFF, 0x0F]);
    let err = decompress(&input).expect_err("a 4099-byte chunk must be refused");
    assert!(
        matches!(&err, VbaError::Compression(msg) if msg.contains("4096")),
        "the per-chunk bomb guard must be the one that fires, got {err:?}"
    );
    // The exhaustive per-branch fixtures (including the global MAX_OUTPUT cap)
    // live in `src/decompress_tests.rs`; this is the panic-freedom smoke.
}

#[test]
fn compress_round_trips_arbitrary_bytes() {
    // The write-back invariant (macro spec §3.4): `decompress ∘ compress == id`
    // for any source bytes, so a save can never corrupt a module.
    let cases: Vec<Vec<u8>> = vec![
        vec![],
        vec![0x00],
        b"Sub X()\r\nEnd Sub\r\n".to_vec(),
        (0..=255u8).collect(),
        vec![0x41u8; 4097], // spans a chunk boundary
        (0u16..9000).map(|i| (i % 256) as u8).collect(),
    ];
    for c in &cases {
        assert_eq!(&decompress(&compress(c)).expect("round-trips"), c);
    }
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
