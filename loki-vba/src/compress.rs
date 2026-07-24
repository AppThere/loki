// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! MS-OVBA compression (`[MS-OVBA] §2.4.1`) — the inverse of [`decompress`].
//!
//! Used by the source-only write-back path (macro spec §3.4): when the macro
//! editor saves an edited module, its source is re-compressed into the
//! `CompressedContainer` a module stream holds. Like [`decompress`], this is a
//! **pure byte transform** — it executes nothing.
//!
//! # Encoding choice — literals-only compressed chunks
//!
//! A `CompressedContainer` is a `0x01` signature byte followed by chunks, each a
//! 2-byte header plus data. This encoder emits every chunk as a **compressed**
//! chunk (`[MS-OVBA] §2.4.1.3.1`) whose token sequences are **all literals** — no
//! copy tokens, so no match-finder. That is deliberately simple *and* the safest
//! correct encoding:
//!
//! - It is not a *raw* (uncompressed) chunk. Raw chunks are fixed at 4096 data
//!   bytes (`§2.4.1.1.5`: `CompressedChunkSize` is always 4095 for them), so a
//!   short final raw chunk is not spec-compliant and a strict reader appends a
//!   full 4096 bytes regardless — corrupting the tail. A literals-only
//!   *compressed* chunk is unambiguously variable-length.
//! - Copy tokens are the only thing whose decode depends on chunk boundaries
//!   (their offset/length split is a function of bytes-so-far *in the chunk*).
//!   With no copy tokens, a chunk decodes identically no matter where it is cut,
//!   so any reader (Office, `LibreOffice`, our own [`decompress`]) reconstructs the
//!   exact bytes.
//!
//! The trade-off is size, not correctness: output is ~1/8 larger than the input
//! plus headers. That is irrelevant for hand-edited macro source, and a real
//! match-finding compressor can replace this later without changing the format.
//!
//! [`decompress`]: crate::decompress::decompress

/// Container signature byte that opens every `CompressedContainer`.
const SIGNATURE_BYTE: u8 = 0x01;

/// Literal decompressed bytes packed into one chunk. Each group of 8 literals
/// costs a 1-byte `FlagByte`, so the compressed data is `n + ceil(n/8)` bytes,
/// which must fit the 12-bit `CompressedChunkSize` field (max 4096 data bytes,
/// `§2.4.1.1.5`). 3584 = 448 groups → 4032 compressed data bytes, safely under
/// the cap and a multiple of 8 (no ragged final group inside a chunk).
const MAX_LITERALS_PER_CHUNK: usize = 3584;

/// High byte pattern of a compressed-chunk header: compressed flag (`0x8000`)
/// OR the mandatory `0b011` signature in bits 14..12 (`0x3000`). The low 12 bits
/// carry `data_len - 1` (`CompressedChunkSize`).
const COMPRESSED_CHUNK_HEADER: u16 = 0x8000 | 0x3000;

/// Compresses raw bytes into an MS-OVBA `CompressedContainer`.
///
/// Infallible: every input encodes (empty input yields the bare signature byte).
/// The result round-trips exactly — `decompress(&compress(x)) == x` for all `x`.
#[must_use]
pub fn compress(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len() + input.len() / 8 + 3);
    out.push(SIGNATURE_BYTE);
    for window in input.chunks(MAX_LITERALS_PER_CHUNK) {
        emit_literal_chunk(window, &mut out);
    }
    out
}

/// Appends one all-literals compressed chunk covering `window` (≤
/// [`MAX_LITERALS_PER_CHUNK`] bytes) to `out`.
fn emit_literal_chunk(window: &[u8], out: &mut Vec<u8>) {
    // Build the chunk data: for each run of up to 8 literals, a 0x00 FlagByte
    // (all eight tokens are literals) followed by the literal bytes themselves.
    let mut data = Vec::with_capacity(window.len() + window.len().div_ceil(8));
    for group in window.chunks(8) {
        data.push(0x00); // FlagByte: every bit 0 ⇒ eight LiteralTokens
        data.extend_from_slice(group);
    }
    // CompressedChunkSize is the data length minus one, in the low 12 bits.
    // `data.len()` is ≤ 4032 by MAX_LITERALS_PER_CHUNK, so this never truncates;
    // `unwrap_or` keeps it panic-free without an `.unwrap()`.
    let size_field = u16::try_from(data.len() - 1).unwrap_or(0x0FFF) & 0x0FFF;
    let header = COMPRESSED_CHUNK_HEADER | size_field;
    out.extend_from_slice(&header.to_le_bytes());
    out.extend_from_slice(&data);
}

#[cfg(test)]
#[path = "compress_tests.rs"]
mod tests;
