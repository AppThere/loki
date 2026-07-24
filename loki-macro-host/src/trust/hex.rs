// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Minimal lowercase-hex encode/decode for 32-byte payload hashes.
//!
//! The trust store is keyed by `MacroPayload::payload_hash()` (`[u8; 32]`). We
//! render it as hex so the on-disk store is human-readable and each key can be
//! its own JSON object field. A dedicated helper avoids a `hex`-crate
//! dependency for this one narrow use.

/// Encodes bytes as a lowercase hex string.
#[must_use]
pub(crate) fn encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        // Two lowercase hex nibbles per byte.
        s.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
        s.push(char::from_digit(u32::from(b & 0x0f), 16).unwrap_or('0'));
    }
    s
}

/// Decodes exactly 64 lowercase/uppercase hex characters into a 32-byte array,
/// or `None` on any length or character error.
#[must_use]
pub(crate) fn decode32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    let bytes = s.as_bytes();
    for (i, slot) in out.iter_mut().enumerate() {
        let hi = (bytes[i * 2] as char).to_digit(16)?;
        let lo = (bytes[i * 2 + 1] as char).to_digit(16)?;
        *slot = ((hi << 4) | lo) as u8;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut bytes = [0u8; 32];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = (i * 7 + 3) as u8;
        }
        let hex = encode(&bytes);
        assert_eq!(hex.len(), 64);
        assert_eq!(decode32(&hex), Some(bytes));
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(decode32(""), None);
        assert_eq!(decode32("zz"), None);
        assert_eq!(decode32(&"0".repeat(63)), None);
        assert_eq!(decode32(&"0".repeat(65)), None);
        assert!(decode32(&"ab".repeat(32)).is_some());
    }
}
