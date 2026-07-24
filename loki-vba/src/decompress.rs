// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! MS-OVBA decompression (`[MS-OVBA] §2.4.1`).
//!
//! The `dir` stream and each module's source stream in a VBA project are stored
//! with a simple LZ77-style RLE compression. This is a **pure byte transform**
//! with hard bomb guards (per-chunk 4096-byte cap + a global output cap); it
//! executes nothing.

use crate::error::{VbaError, VbaResult};

/// Global cap on decompressed output — a decompression-bomb guard (spec §8).
const MAX_OUTPUT: usize = 64 * 1024 * 1024;
/// A single chunk decompresses to at most 4096 bytes (`[MS-OVBA] §2.4.1.1.3`).
const CHUNK_LIMIT: usize = 4096;

/// Decompresses an MS-OVBA `CompressedContainer` to its raw bytes.
///
/// # Errors
///
/// [`VbaError::Compression`] on a malformed container, [`VbaError::TooLarge`]
/// if the output exceeds the bomb-guard cap.
pub fn decompress(input: &[u8]) -> VbaResult<Vec<u8>> {
    let mut it = input.iter();
    match it.next() {
        Some(0x01) => {}
        _ => return Err(VbaError::Compression("missing 0x01 signature byte".into())),
    }
    let mut pos = 1usize;
    let mut out = Vec::new();
    while pos < input.len() {
        pos = decompress_one_chunk(input, pos, &mut out)?;
        if out.len() > MAX_OUTPUT {
            return Err(VbaError::TooLarge);
        }
    }
    Ok(out)
}

/// Decompresses one chunk starting at `pos`, returning the position after it.
fn decompress_one_chunk(input: &[u8], pos: usize, out: &mut Vec<u8>) -> VbaResult<usize> {
    let Some(header_bytes) = input.get(pos..pos + 2) else {
        return Err(VbaError::Compression("truncated chunk header".into()));
    };
    let header = u16::from_le_bytes([header_bytes[0], header_bytes[1]]);
    let compressed = header & 0x8000 != 0;
    let signature = (header >> 12) & 0x7;
    if signature != 0b011 {
        return Err(VbaError::Compression("bad chunk signature".into()));
    }
    let data_size = (header & 0x0FFF) as usize + 1;
    let data_start = pos + 2;
    let data_end = data_start + data_size;
    let Some(data) = input.get(data_start..data_end) else {
        return Err(VbaError::Compression("truncated chunk data".into()));
    };

    if compressed {
        decompress_chunk(data, out)?;
    } else {
        // Raw chunk: the data is copied verbatim (always 4096 bytes in a
        // well-formed container; we copy whatever the header declares).
        out.extend_from_slice(data);
    }
    Ok(data_end)
}

fn decompress_chunk(data: &[u8], out: &mut Vec<u8>) -> VbaResult<()> {
    let chunk_start = out.len();
    let mut i = 0usize;
    while i < data.len() {
        let flags = data[i];
        i += 1;
        for bit in 0..8 {
            if i >= data.len() {
                return Ok(());
            }
            if flags & (1 << bit) == 0 {
                out.push(data[i]);
                i += 1;
            } else {
                i = copy_token(data, i, out, chunk_start)?;
            }
            if out.len() - chunk_start > CHUNK_LIMIT {
                return Err(VbaError::Compression("chunk exceeded 4096 bytes".into()));
            }
        }
    }
    Ok(())
}

/// Applies one 2-byte copy token, returning the new read position.
fn copy_token(data: &[u8], i: usize, out: &mut Vec<u8>, chunk_start: usize) -> VbaResult<usize> {
    let Some(bytes) = data.get(i..i + 2) else {
        return Err(VbaError::Compression("truncated copy token".into()));
    };
    let token = u16::from_le_bytes([bytes[0], bytes[1]]);
    let decompressed = out.len() - chunk_start;
    let bit_count = bit_count(decompressed);
    let length_mask = 0xFFFFu16 >> bit_count;
    let offset_mask = !length_mask;
    let length = (token & length_mask) as usize + 3;
    let offset = ((token & offset_mask) >> (16 - bit_count)) as usize + 1;
    if offset > decompressed {
        return Err(VbaError::Compression(
            "copy offset precedes chunk start".into(),
        ));
    }
    let start = out.len() - offset;
    for k in 0..length {
        let b = out[start + k];
        out.push(b);
    }
    Ok(i + 2)
}

/// The offset/length split bit count for a copy token, given how many bytes are
/// already decompressed in the current chunk (`[MS-OVBA] §2.4.1.3.19.3`).
fn bit_count(decompressed_current: usize) -> u32 {
    let mut bits = 4u32;
    while (1usize << bits) < decompressed_current {
        bits += 1;
    }
    bits.min(12)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals_only() {
        // chunk data = [flag 0x00, 'A','B','C']; header 0xB003.
        let input = [0x01, 0x03, 0xB0, 0x00, 0x41, 0x42, 0x43];
        assert_eq!(decompress(&input).unwrap(), b"ABC");
    }

    #[test]
    fn copy_token_repeats() {
        // [flag 0x08, 'A','B','C', copy(len=3,off=3)=0x2000] → "ABCABC".
        let input = [0x01, 0x05, 0xB0, 0x08, 0x41, 0x42, 0x43, 0x00, 0x20];
        assert_eq!(decompress(&input).unwrap(), b"ABCABC");
    }

    #[test]
    fn overlapping_copy_is_run_length() {
        // [flag 0x02, 'A', copy(len=3,off=1)=0x0000] → "AAAA".
        let input = [0x01, 0x03, 0xB0, 0x02, 0x41, 0x00, 0x00];
        assert_eq!(decompress(&input).unwrap(), b"AAAA");
    }

    #[test]
    fn missing_signature_is_error() {
        assert!(decompress(&[0x00, 0x01, 0x02]).is_err());
    }

    #[test]
    fn truncated_header_is_error() {
        assert!(decompress(&[0x01, 0x03]).is_err());
    }
}
