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

    /// Every case in `rejects_bad_input` is caught by the **length** guard, so
    /// the `to_digit(16)?` character check never runs there: deleting it (or
    /// substituting a wrapping cast) fails no test. These inputs are exactly 64
    /// characters, so they reach that branch.
    #[test]
    fn rejects_correct_length_with_a_non_hex_character() {
        // 63 zeros + 'g' — the character guard is the only thing that can
        // reject this.
        assert_eq!(decode32(&("0".repeat(63) + "g")), None);
        // First character rather than last, so the loop's very first read is
        // covered too.
        assert_eq!(decode32(&("g".to_string() + &"0".repeat(63))), None);
        // A non-ASCII byte: `s.len()` is in bytes, so this is 64 bytes but 62
        // characters. It must not panic and must not decode.
        assert_eq!(decode32(&("é".to_string() + &"0".repeat(62))), None);
        // Control: the same shape with a legal hex digit does decode, so the
        // three rejections above are about the character, not the length.
        assert!(decode32(&("0".repeat(63) + "f")).is_some());
    }

    /// `roundtrip` is symmetric — a consistent nibble-order swap in *both*
    /// functions passes it, while corrupting interop with any store written by
    /// a correct implementation (payload-hash keys would no longer match, so
    /// every persisted trust record would be orphaned).
    ///
    /// These vectors are hand-derived, not produced by the code under test.
    #[test]
    fn encode_matches_the_golden_vector() {
        // High nibble first, lowercase — the property a swap breaks.
        assert_eq!(encode(&[0xDE, 0xAD, 0xBE, 0xEF]), "deadbeef");
        assert_eq!(encode(&[0x01, 0x0F, 0xF0, 0x10]), "010ff010");
        assert_eq!(encode(&[]), "");

        // Full 32-byte key: bytes 0x00..=0x1F render as "000102…1f".
        let mut key = [0u8; 32];
        for (i, b) in key.iter_mut().enumerate() {
            *b = i as u8;
        }
        assert_eq!(
            encode(&key),
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
        );
    }

    /// The decode direction against the same hand-derived vectors, plus the
    /// documented uppercase tolerance (which no other test exercises).
    #[test]
    fn decode32_matches_the_golden_vector() {
        let mut expected = [0u8; 32];
        for (i, b) in expected.iter_mut().enumerate() {
            *b = i as u8;
        }
        assert_eq!(
            decode32("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"),
            Some(expected)
        );
        // Uppercase input decodes to the same bytes (the doc comment promises
        // "lowercase/uppercase"); `encode` still emits lowercase.
        assert_eq!(
            decode32("000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F"),
            Some(expected)
        );

        // Nibble order: "de" is 0xDE, not 0xED.
        let de = decode32(&("de".to_string() + &"00".repeat(31))).expect("valid hex");
        assert_eq!(de[0], 0xDE);
    }
}
