// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

// Test fixtures build byte patterns with `i as u8` index truncation on purpose
// (deterministic filler); the narrowing is intentional, not a bug.
#![allow(clippy::cast_possible_truncation)]

use crate::decompress::decompress;

use super::{MAX_LITERALS_PER_CHUNK, compress};

/// `compress` then `decompress` must reproduce the input exactly.
fn assert_round_trips(input: &[u8]) {
    let packed = compress(input);
    assert_eq!(packed.first(), Some(&0x01), "container must open with 0x01");
    let back = decompress(&packed).expect("our own container must decompress");
    assert_eq!(back, input, "round-trip mismatch for {} bytes", input.len());
}

#[test]
fn empty_input_is_bare_signature() {
    assert_eq!(compress(b""), vec![0x01]);
    assert_round_trips(b"");
}

#[test]
fn abc_matches_the_decompressor_test_vector() {
    // The decompressor's `literals_only` fixture, produced from the other side:
    // [0x01, header=0xB003 LE, FlagByte 0x00, 'A','B','C'].
    assert_eq!(
        compress(b"ABC"),
        vec![0x01, 0x03, 0xB0, 0x00, 0x41, 0x42, 0x43]
    );
    assert_round_trips(b"ABC");
}

#[test]
fn group_boundary_lengths_round_trip() {
    // 7 (partial group), 8 (exact group), 9 (group + 1) exercise the 8-literal
    // FlagByte packing edges.
    for n in [1usize, 7, 8, 9, 15, 16, 17] {
        let input: Vec<u8> = (0..n).map(|i| i as u8).collect();
        assert_round_trips(&input);
    }
}

#[test]
fn chunk_boundary_lengths_round_trip() {
    // Around the per-chunk literal cap: last-of-chunk, exactly full, spill into
    // a second chunk, and several chunks.
    for n in [
        MAX_LITERALS_PER_CHUNK - 1,
        MAX_LITERALS_PER_CHUNK,
        MAX_LITERALS_PER_CHUNK + 1,
    ] {
        let input: Vec<u8> = (0..n).map(|i| (i % 251) as u8).collect();
        assert_round_trips(&input);
    }
    let three_chunks: Vec<u8> = (0..MAX_LITERALS_PER_CHUNK * 3 + 5)
        .map(|i| (i % 256) as u8)
        .collect();
    assert_round_trips(&three_chunks);
}

#[test]
fn all_byte_values_and_runs_round_trip() {
    let all_bytes: Vec<u8> = (0..=255u8).collect();
    assert_round_trips(&all_bytes);
    assert_round_trips(&vec![0xAAu8; 5000]); // long single-byte run
    assert_round_trips(&vec![0x00u8; MAX_LITERALS_PER_CHUNK + 1]);
}

#[test]
fn realistic_vba_source_round_trips() {
    let src = "Attribute VB_Name = \"Module1\"\r\n\
               Sub RunReport()\r\n\
               \x20\x20\x20\x20MsgBox \"Hello, \" & Application.Name\r\n\
               End Sub\r\n";
    assert_round_trips(src.as_bytes());
}

/// Every emitted chunk must carry the `0b011` signature and a `CompressedChunkSize`
/// within the 12-bit field (≤ 4096 data bytes) — otherwise a strict MS-OVBA
/// reader (Office / `LibreOffice`) would reject the container.
#[test]
fn emitted_chunks_are_structurally_valid() {
    let input: Vec<u8> = (0..MAX_LITERALS_PER_CHUNK * 2 + 17)
        .map(|i| (i % 256) as u8)
        .collect();
    let packed = compress(&input);
    assert_eq!(packed[0], 0x01);

    let mut pos = 1usize;
    let mut chunks = 0usize;
    while pos < packed.len() {
        let header = u16::from_le_bytes([packed[pos], packed[pos + 1]]);
        assert!(header & 0x8000 != 0, "chunk must be marked compressed");
        assert_eq!((header >> 12) & 0x7, 0b011, "chunk signature must be 0b011");
        let data_len = (header & 0x0FFF) as usize + 1;
        assert!(
            data_len <= 4096,
            "compressed data must fit the 12-bit field"
        );
        pos += 2 + data_len;
        chunks += 1;
    }
    assert_eq!(pos, packed.len(), "chunks must tile the container exactly");
    assert_eq!(chunks, 3, "two full chunks + a short tail chunk");
}
