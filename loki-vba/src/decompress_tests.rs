// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for the MS-OVBA decompressor.
//!
//! Fixtures are hand-built containers, never produced by our own compressor:
//! the compressor emits only well-formed, compressed chunks, so a round-trip
//! can reach neither the raw-chunk branch nor any error branch. Every case here
//! asserts the **specific** typed error (or the exact bytes), so swapping a
//! branch to a silent `Ok` — truncation-tolerant parsing, the classic
//! malware-evasion vector in OVBA readers — fails a test.

use super::{CHUNK_LIMIT, MAX_OUTPUT, decompress};
use crate::error::VbaError;

// ── Container construction helpers ──────────────────────────────────────────

/// A chunk header: `0b011` signature, the compressed bit, and `len - 1`.
fn header(compressed: bool, data_len: usize) -> [u8; 2] {
    let sig = 0x3000u16;
    let bit = if compressed { 0x8000 } else { 0 };
    #[allow(clippy::cast_possible_truncation)] // callers stay well under 4096
    let size = (data_len - 1) as u16;
    (sig | bit | size).to_le_bytes()
}

/// A container holding one chunk of `data`.
fn container(compressed: bool, data: &[u8]) -> Vec<u8> {
    let mut v = vec![0x01u8];
    v.extend_from_slice(&header(compressed, data.len()));
    v.extend_from_slice(data);
    v
}

/// A copy token for `length` bytes at `offset`, valid only while fewer than 16
/// bytes are decompressed in the chunk (so the split is 4 offset bits).
fn copy_token_4bit(offset: u16, length: u16) -> [u8; 2] {
    (((offset - 1) << 12) | (length - 3)).to_le_bytes()
}

/// Chunk data that expands to `1 + run` bytes: one literal `A`, then a
/// single overlapping copy that run-length-extends it.
fn literal_then_run(run: u16) -> Vec<u8> {
    let token = copy_token_4bit(1, run);
    vec![0x02, b'A', token[0], token[1]]
}

/// The `VbaError` a container must fail with, or a panic naming what came back.
fn error_of(input: &[u8]) -> VbaError {
    match decompress(input) {
        Ok(out) => panic!("expected an error, decompressed {} bytes", out.len()),
        Err(e) => e,
    }
}

/// The message of a [`VbaError::Compression`], or a panic naming the variant.
fn compression_message(input: &[u8]) -> String {
    match error_of(input) {
        VbaError::Compression(msg) => msg,
        other => panic!("expected VbaError::Compression, got {other:?}"),
    }
}

// ── Happy path ──────────────────────────────────────────────────────────────

#[test]
fn literals_only() {
    // chunk data = [flag 0x00, 'A','B','C']; header 0xB003.
    let input = [0x01, 0x03, 0xB0, 0x00, 0x41, 0x42, 0x43];
    assert_eq!(decompress(&input).expect("well-formed"), b"ABC");
}

#[test]
fn copy_token_repeats() {
    // [flag 0x08, 'A','B','C', copy(len=3,off=3)=0x2000] → "ABCABC".
    let input = [0x01, 0x05, 0xB0, 0x08, 0x41, 0x42, 0x43, 0x00, 0x20];
    assert_eq!(decompress(&input).expect("well-formed"), b"ABCABC");
}

#[test]
fn overlapping_copy_is_run_length() {
    // [flag 0x02, 'A', copy(len=3,off=1)=0x0000] → "AAAA".
    let input = [0x01, 0x03, 0xB0, 0x02, 0x41, 0x00, 0x00];
    assert_eq!(decompress(&input).expect("well-formed"), b"AAAA");
}

// ── Raw (uncompressed) chunks — the `compressed == false` branch ─────────────

#[test]
fn raw_chunk_is_copied_verbatim() {
    // Real Office files emit raw chunks for incompressible data; our compressor
    // never does, so only a hand-built container reaches this branch.
    let payload = b"RAW!";
    assert_eq!(
        decompress(&container(false, payload)).expect("raw chunk"),
        payload
    );
}

#[test]
fn raw_chunk_bytes_are_not_read_as_compression_tokens() {
    // Inversion for the branch above: the *same* payload with the compressed bit
    // set is read as flags + tokens instead, and this one is then malformed. So
    // the verbatim copy is doing real work, not agreeing by coincidence.
    let payload = b"RAW!";
    assert!(
        matches!(
            error_of(&container(true, payload)),
            VbaError::Compression(_)
        ),
        "the compressed reading of this payload must not also succeed"
    );
}

#[test]
fn raw_chunk_preserves_bytes_that_look_like_tokens() {
    // A payload of 0xFF bytes would be eight copy tokens if misrouted.
    let payload = [0xFFu8; 32];
    assert_eq!(
        decompress(&container(false, &payload)).expect("raw chunk"),
        payload
    );
}

// ── Bomb guards ─────────────────────────────────────────────────────────────

#[test]
fn chunk_expanding_past_4096_bytes_is_refused() {
    // A *reachable* chunk-limit case: one literal seeds the window, then one
    // maximal overlapping copy (offset 1, length 4098) expands it for real. The
    // guard must stop it — 4099 > CHUNK_LIMIT.
    let msg = compression_message(&container(true, &literal_then_run(4098)));
    assert!(
        msg.contains("4096"),
        "the 4096-byte chunk guard must be the one that fires, got {msg:?}"
    );
    assert_eq!(CHUNK_LIMIT, 4096, "the fixture is sized against this cap");
}

#[test]
fn chunk_expanding_to_exactly_4096_bytes_is_allowed() {
    // Floor for the guard above (a counting gate needs both bounds): one byte
    // less must decompress cleanly, so the cap cannot be quietly tightened.
    let out = decompress(&container(true, &literal_then_run(4095))).expect("exactly at the cap");
    assert_eq!(out.len(), CHUNK_LIMIT);
    assert!(out.iter().all(|&b| b == b'A'));
}

#[test]
fn output_past_the_global_cap_is_too_large() {
    // Many *individually legal* chunks (each exactly at CHUNK_LIMIT, so the
    // per-chunk guard never fires) crossing MAX_OUTPUT: this must be stopped by
    // the global cap, and reported as TooLarge rather than Compression.
    let chunk_data = literal_then_run(4095);
    let chunks = MAX_OUTPUT / CHUNK_LIMIT + 1;
    let mut input = vec![0x01u8];
    for _ in 0..chunks {
        input.extend_from_slice(&header(true, chunk_data.len()));
        input.extend_from_slice(&chunk_data);
    }
    assert_eq!(
        error_of(&input),
        VbaError::TooLarge,
        "the global output cap must be the guard that fires"
    );
}

// ── Malformed containers — one fixture per error branch ─────────────────────

#[test]
fn missing_signature_is_a_compression_error() {
    assert!(
        compression_message(&[0x00, 0x01, 0x02]).contains("signature byte"),
        "the leading 0x01 check must be the branch that fires"
    );
}

#[test]
fn truncated_header_is_a_compression_error() {
    assert!(
        compression_message(&[0x01, 0x03]).contains("truncated chunk header"),
        "a 1-byte header must be reported as a truncated header"
    );
}

#[test]
fn bad_chunk_signature_is_rejected() {
    // Signature bits 0b000 instead of 0b011, compressed bit set.
    let input = [0x01, 0x03, 0x80];
    assert!(
        compression_message(&input).contains("bad chunk signature"),
        "a wrong 3-bit chunk signature must be rejected, not parsed anyway"
    );
}

#[test]
fn truncated_chunk_data_is_rejected() {
    // Header declares 4 data bytes; only 1 follows. Tolerating this is how a
    // reader can be made to disagree with the engine about a module's source.
    let input = [0x01, 0x03, 0xB0, 0x00];
    assert!(
        compression_message(&input).contains("truncated chunk data"),
        "a chunk shorter than its declared size must be rejected"
    );
}

#[test]
fn truncated_copy_token_is_rejected() {
    // flags mark a copy, but only one of the token's two bytes is present.
    let input = [0x01, 0x02, 0xB0, 0x02, 0x41, 0x00];
    assert!(
        compression_message(&input).contains("truncated copy token"),
        "a 1-byte copy token must be rejected, not silently ignored"
    );
}

#[test]
fn copy_offset_before_the_chunk_start_is_rejected() {
    // One literal decompressed, then a copy reaching 3 bytes back — outside the
    // chunk window, i.e. into bytes from a *previous* chunk.
    let token = copy_token_4bit(3, 3);
    let input = container(true, &[0x02, b'A', token[0], token[1]]);
    assert!(
        compression_message(&input).contains("copy offset precedes chunk start"),
        "a copy reaching before the chunk start must be rejected"
    );
}

#[test]
fn copy_token_with_no_prior_output_is_rejected() {
    // The degenerate form of the same guard: the first token in a chunk, with
    // nothing decompressed yet, can never have a valid offset.
    let input = [0x01, 0x02, 0xB0, 0x01, 0x00, 0x00];
    assert!(
        compression_message(&input).contains("copy offset precedes chunk start"),
        "a leading copy token must be rejected"
    );
}
