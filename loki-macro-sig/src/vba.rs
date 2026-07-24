// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! VBA signature-stream location + discrimination (8A.2; MS-OVBA / MS-OSHARED).
//!
//! A signed `vbaProject.bin` carries up to three signature streams at the OLE
//! root: `\x05DigitalSignature` (legacy, MD5-based), `\x05DigitalSignatureAgile`,
//! and `\x05DigitalSignatureV3`. Each is an MS-OSHARED `DigSigInfoSerialized`
//! wrapper around a PKCS#7 `SignedData`.
//!
//! This module does the **reliable, total** part of parsing: find which streams
//! are present, discriminate the variant (names are well-defined), return each
//! raw stream, and locate the embedded PKCS#7 `ContentInfo` by its `signedData`
//! OID via a bounded DER scan. The scan is robust to the exact MS-OSHARED wrapper
//! offsets — which vary by variant and are validated against a real corpus when
//! the verifier lands.
//!
//! TODO(8A.3-corpus): cross-check the DER-located PKCS#7 against the structured
//! `DigSigInfoSerialized` `signatureOffset`/`cbSignature` fields once the
//! `RustCrypto` verifier is validated on real signed `.docm` samples.
//!
//! Everything here is a pure byte transform over an in-memory buffer, total on
//! hostile input (malformed → empty / `None`, never a panic — T9).

use std::io::{Cursor, Read};
use std::path::PathBuf;

/// Which VBA signature variant a stream holds (ADR-0014 §4.2). Ordered strongest
/// first — only the SHA-2 variants (`Agile`/`V3`) are eligible for trust; the
/// MD5-based `Legacy` never grants it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SigVariant {
    /// `\x05DigitalSignatureV3` — the strongest.
    V3,
    /// `\x05DigitalSignatureAgile`.
    Agile,
    /// `\x05DigitalSignature` — legacy MD5; never trusted.
    Legacy,
}

impl SigVariant {
    /// Maps a compound-file stream leaf name (including its `\x05` prefix) to a
    /// variant, or `None` for a non-signature stream.
    #[must_use]
    fn from_stream_name(name: &str) -> Option<Self> {
        match name {
            "\u{5}DigitalSignatureV3" => Some(SigVariant::V3),
            "\u{5}DigitalSignatureAgile" => Some(SigVariant::Agile),
            "\u{5}DigitalSignature" => Some(SigVariant::Legacy),
            _ => None,
        }
    }

    /// Whether this variant is eligible to grant trust (SHA-2 family). The legacy
    /// MD5 signature is displayed but never trusted (downgrade defence).
    #[must_use]
    pub fn is_trust_eligible(self) -> bool {
        matches!(self, SigVariant::V3 | SigVariant::Agile)
    }
}

/// One located signature stream: its variant and raw bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawVbaSignature {
    /// The signature variant.
    pub variant: SigVariant,
    /// The raw stream contents (the MS-OSHARED wrapper + PKCS#7).
    pub stream: Vec<u8>,
}

impl RawVbaSignature {
    /// The embedded PKCS#7 `SignedData` as a DER slice, located by scanning for
    /// the `ContentInfo` whose type is `signedData` (OID 1.2.840.113549.1.7.2).
    /// `None` if the stream holds no such structure. This is what 8A.3 hands to
    /// the CMS verifier.
    #[must_use]
    pub fn pkcs7_der(&self) -> Option<&[u8]> {
        find_signed_data_content_info(&self.stream)
    }
}

/// Extracts every VBA signature stream from a `vbaProject.bin`, strongest variant
/// first. Returns empty for a non-compound-file input or one with no signature
/// streams — total on hostile input.
#[must_use]
pub fn extract_vba_signatures(vba_project_bin: &[u8]) -> Vec<RawVbaSignature> {
    let Ok(mut comp) = cfb::CompoundFile::open(Cursor::new(vba_project_bin)) else {
        return Vec::new();
    };
    // Collect (variant, path) first so the immutable walk borrow ends before the
    // mutable stream reads.
    let mut located: Vec<(SigVariant, PathBuf)> = comp
        .walk()
        .filter(cfb::Entry::is_stream)
        .filter_map(|e| SigVariant::from_stream_name(e.name()).map(|v| (v, e.path().to_path_buf())))
        .collect();
    located.sort_by_key(|(v, _)| *v); // strongest first (SigVariant Ord)

    let mut out = Vec::with_capacity(located.len());
    for (variant, path) in located {
        if let Ok(mut stream) = comp.open_stream(&path) {
            let mut buf = Vec::new();
            if stream.read_to_end(&mut buf).is_ok() {
                out.push(RawVbaSignature {
                    variant,
                    stream: buf,
                });
            }
        }
    }
    out
}

/// DER prefix of a `ContentInfo` whose `contentType` is PKCS#7 `signedData`:
/// `OID (06 09) 1.2.840.113549.1.7.2`.
const SIGNED_DATA_OID: [u8; 11] = [
    0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x07, 0x02,
];

/// Finds the first DER `SEQUENCE` whose content begins with the `signedData` OID
/// and returns the whole element (tag..end). Bounds-checked and total.
fn find_signed_data_content_info(blob: &[u8]) -> Option<&[u8]> {
    let mut i = 0usize;
    while i < blob.len() {
        if blob[i] == 0x30
            && let Some((header, len)) = der_length(blob, i + 1)
            && let Some(end) = (i + 1 + header).checked_add(len)
            && end <= blob.len()
            && blob[i + 1 + header..end].starts_with(&SIGNED_DATA_OID)
        {
            return Some(&blob[i..end]);
        }
        i += 1;
    }
    None
}

/// Reads a DER length field at `p`, returning `(bytes_consumed, length)`.
/// Rejects the indefinite form and absurd (> 4-byte) lengths. Bounds-checked.
fn der_length(blob: &[u8], p: usize) -> Option<(usize, usize)> {
    let first = *blob.get(p)?;
    if first < 0x80 {
        return Some((1, first as usize));
    }
    let n = (first & 0x7F) as usize;
    if n == 0 || n > 4 {
        return None; // indefinite form or an implausibly large length
    }
    let mut len = 0usize;
    for k in 0..n {
        len = (len << 8) | *blob.get(p + 1 + k)? as usize;
    }
    Some((1 + n, len))
}

#[cfg(test)]
#[path = "vba_tests.rs"]
mod tests;
