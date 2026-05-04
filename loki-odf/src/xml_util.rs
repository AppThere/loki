// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Shared XML parsing utilities for ODF part readers.
// Items in this module are pub(crate) utility functions consumed by importer
// sub-modules added in later sessions; suppress the premature dead_code lint.
#![allow(dead_code)]
//!
//! ODF measurement values are expressed as strings with explicit unit
//! suffixes (e.g. `"2.5cm"`, `"1in"`, `"12pt"`, `"10mm"`), unlike OOXML
//! which uses bare integer twips or EMUs. [`parse_length`] handles this
//! conversion.
//!
//! ODF colors use a `#RRGGBB` hash-prefixed hex format (unlike OOXML which
//! omits the hash). [`parse_hex_color`] strips the leading `#` before parsing.
//!
//! # Why `trim_text(false)` must always be set
//!
//! `quick_xml` can strip leading/trailing whitespace from text nodes when
//! `trim_text(true)` is set. ODF documents use `xml:space="preserve"` and
//! rely on significant whitespace within text spans. Trimming would silently
//! discard inter-word spaces. Every reader in this crate must call
//! `reader.config_mut().trim_text(false)` immediately after construction.

use appthere_color::RgbColor;
use loki_primitives::units::Points;
use quick_xml::events::BytesStart;

// ── Length parsing ─────────────────────────────────────────────────────────────

/// Points per centimetre: 1 cm = 28.3465 pt (72 pt/in ÷ 2.54 cm/in).
const PT_PER_CM: f64 = 72.0 / 2.54;

/// Points per millimetre: 1 mm = 2.83465 pt.
const PT_PER_MM: f64 = 72.0 / 25.4;

/// Points per inch: 1 in = 72 pt.
const PT_PER_IN: f64 = 72.0;

/// Parse an ODF length string into [`Points`].
///
/// Recognised unit suffixes (ODF 1.3 §18.3.18 `length`):
/// - `"pt"` — typographic points
/// - `"cm"` — centimetres (1 cm = 28.3465 pt)
/// - `"mm"` — millimetres (1 mm = 2.83465 pt)
/// - `"in"` — inches (1 in = 72 pt)
///
/// Returns `None` for an empty string, an unrecognised suffix, or a value
/// that cannot be parsed as `f64`.
#[must_use]
pub fn parse_length(s: &str) -> Option<Points> {
    if s.is_empty() {
        return None;
    }

    let (num_str, factor) = if let Some(n) = s.strip_suffix("pt") {
        (n, 1.0_f64)
    } else if let Some(n) = s.strip_suffix("cm") {
        (n, PT_PER_CM)
    } else if let Some(n) = s.strip_suffix("mm") {
        (n, PT_PER_MM)
    } else if let Some(n) = s.strip_suffix("in") {
        (n, PT_PER_IN)
    } else {
        return None;
    };

    let value: f64 = num_str.trim().parse().ok()?;
    Some(Points::new(value * factor))
}

// ── Attribute helpers ──────────────────────────────────────────────────────────

/// Extracts the value of an attribute by its local name (without namespace
/// prefix).
///
/// Returns `None` if the attribute is absent or its value is not valid UTF-8.
///
/// ODF elements use many namespace prefixes (`text:`, `style:`, `fo:`, etc.).
/// This function matches on the local part only so callers do not need to
/// track which prefix a particular attribute was declared under.
///
/// # Examples
///
/// ```ignore
/// // <text:p text:style-name="Body_20_Text"/>
/// // → local_attr_val(e, b"style-name") = Some("Body_20_Text")
/// ```
#[must_use]
pub fn local_attr_val(e: &BytesStart<'_>, local: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|attr| {
        let key_bytes = attr.key.as_ref();
        let key_local = if let Some(pos) =
            key_bytes.iter().position(|&b| b == b':')
        {
            &key_bytes[pos + 1..]
        } else {
            key_bytes
        };
        if key_local == local {
            attr.unescape_value().ok().map(std::borrow::Cow::into_owned)
        } else {
            None
        }
    })
}

// ── Color parsing ──────────────────────────────────────────────────────────────

/// Parses an ODF `#RRGGBB` hex color string to an [`RgbColor`].
///
/// ODF stores colors with a leading `#` (e.g. `fo:color="#FF0000"`).
/// This function strips the `#` then delegates to the six-digit hex parser.
///
/// Returns `None` if the string does not start with `#` or if the hex
/// digits are invalid.
///
/// ODF 1.3 §18.3.9 (`color`).
#[must_use]
pub fn parse_hex_color(s: &str) -> Option<RgbColor> {
    let hex = s.strip_prefix('#')?;
    hex_color(hex)
}

/// Parses a 6-character hexadecimal color string (no `#` prefix) to an
/// [`RgbColor`].
///
/// Returns `None` for the empty string, the value `"auto"`, or any string
/// that is not exactly 6 valid hex digits.
///
/// This is a lower-level helper; prefer [`parse_hex_color`] for ODF attribute
/// values (which include a `#` prefix).
#[must_use]
pub fn hex_color(s: &str) -> Option<RgbColor> {
    if s == "auto" || s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(RgbColor::new(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
    ))
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_length ──────────────────────────────────────────────────────

    #[test]
    fn parse_length_pt() {
        let pts = parse_length("12pt").unwrap();
        assert!((pts.value() - 12.0).abs() < 1e-9);
    }

    #[test]
    fn parse_length_in() {
        let pts = parse_length("1in").unwrap();
        assert!((pts.value() - 72.0).abs() < 1e-9);
    }

    #[test]
    fn parse_length_cm() {
        // 2.5 cm × (72/2.54) pt/cm ≈ 70.866 pt
        let pts = parse_length("2.5cm").unwrap();
        assert!((pts.value() - 70.866).abs() < 0.001, "got {}", pts.value());
    }

    #[test]
    fn parse_length_mm() {
        // 10 mm × (72/25.4) pt/mm ≈ 28.346 pt
        let pts = parse_length("10mm").unwrap();
        assert!((pts.value() - 28.346).abs() < 0.001, "got {}", pts.value());
    }

    #[test]
    fn parse_length_fractional_in() {
        let pts = parse_length("0.5in").unwrap();
        assert!((pts.value() - 36.0).abs() < 1e-9);
    }

    #[test]
    fn parse_length_empty_is_none() {
        assert!(parse_length("").is_none());
    }

    #[test]
    fn parse_length_unknown_unit_is_none() {
        assert!(parse_length("10px").is_none());
        assert!(parse_length("5em").is_none());
    }

    #[test]
    fn parse_length_invalid_number_is_none() {
        assert!(parse_length("abcpt").is_none());
    }

    // ── parse_hex_color ───────────────────────────────────────────────────

    #[test]
    fn parse_hex_color_red() {
        let c = parse_hex_color("#FF0000").unwrap();
        assert!((c.red() - 1.0_f32).abs() < 1e-6);
        assert!(c.green() < 1e-6);
        assert!(c.blue() < 1e-6);
    }

    #[test]
    fn parse_hex_color_black() {
        let c = parse_hex_color("#000000").unwrap();
        assert!(c.red() < 1e-6);
        assert!(c.green() < 1e-6);
        assert!(c.blue() < 1e-6);
    }

    #[test]
    fn parse_hex_color_white() {
        let c = parse_hex_color("#FFFFFF").unwrap();
        assert!((c.red() - 1.0_f32).abs() < 1e-6);
        assert!((c.green() - 1.0_f32).abs() < 1e-6);
        assert!((c.blue() - 1.0_f32).abs() < 1e-6);
    }

    #[test]
    fn parse_hex_color_missing_hash_is_none() {
        assert!(parse_hex_color("FF0000").is_none());
    }

    #[test]
    fn parse_hex_color_empty_is_none() {
        assert!(parse_hex_color("").is_none());
    }

    #[test]
    fn parse_hex_color_lowercase() {
        let c = parse_hex_color("#ff8000").unwrap();
        assert!((c.red() - 1.0_f32).abs() < 1e-6);
        assert!((c.green() - 128.0_f32 / 255.0).abs() < 1e-4);
        assert!(c.blue() < 1e-6);
    }

    // ── hex_color ─────────────────────────────────────────────────────────

    #[test]
    fn hex_color_auto_is_none() {
        assert!(hex_color("auto").is_none());
    }

    #[test]
    fn hex_color_wrong_length_is_none() {
        assert!(hex_color("FFF").is_none());
        assert!(hex_color("FFFFFFF").is_none());
    }
}
