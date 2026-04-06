// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Shared XML parsing utilities for all OOXML part readers.
//!
//! # Why `trim_text(false)` must always be set
//!
//! `quick_xml` can strip leading/trailing whitespace from text nodes when
//! `trim_text(true)` is set. OOXML documents frequently use `xml:space="preserve"`
//! and rely on significant whitespace within runs (`<w:t xml:space="preserve"> </w:t>`).
//! Trimming would silently discard inter-word spaces, producing garbled text.
//! Every reader in this crate must call `reader.config_mut().trim_text(false)`
//! immediately after constructing the reader.

use appthere_color::RgbColor;
use loki_primitives::units::Points;
use quick_xml::events::BytesStart;

use crate::constants::{EMUS_PER_PT, HALF_POINTS_PER_PT, TWIPS_PER_PT};

/// Extracts the local name (without namespace prefix) from an element.
///
/// OOXML uses many namespace prefixes (`w:`, `wp:`, `a:`, `r:`, etc.).
/// This crate matches on local names only (ECMA-376 §L.5).
///
/// # Examples
///
/// ```ignore
/// // b"w:p" → b"p"
/// // b"a:blip" → b"blip"
/// // b"numFmt" → b"numFmt"
/// ```
#[must_use]
pub fn local_name<'a>(e: &'a BytesStart<'a>) -> &'a [u8] {
    let bytes = e.local_name().into_inner();
    if let Some(pos) = bytes.iter().position(|&b| b == b':') {
        &bytes[pos + 1..]
    } else {
        bytes
    }
}

/// Extracts the value of an attribute by its local name (without prefix).
///
/// Returns `None` if the attribute is absent or its value is not valid UTF-8.
///
/// # Examples
///
/// ```ignore
/// // <w:numFmt w:val="decimal"/> → local_attr_val(e, b"val") = Some("decimal")
/// ```
#[must_use]
pub fn local_attr_val(e: &BytesStart<'_>, local: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|attr| {
        let key_bytes = attr.key.as_ref();
        let key_local = if let Some(pos) = key_bytes.iter().position(|&b| b == b':') {
            &key_bytes[pos + 1..]
        } else {
            key_bytes
        };
        if key_local == local {
            attr.unescape_value().ok().map(|v| v.into_owned())
        } else {
            None
        }
    })
}

/// Parses an OOXML toggle-property `@w:val` string to a `bool`.
///
/// Toggle properties follow ECMA-376 §17.7.3: the element being present
/// with no `@w:val` or `@w:val="1"/"true"/"on"` means `true`;
/// `@w:val="0"/"false"/"off"` means `false`.
///
/// This function is for callers that have already retrieved the attribute
/// value. Use [`crate::docx::reader::util::toggle_prop`] when the attribute
/// may be absent (returns `Option<bool>` instead).
#[must_use]
pub fn bool_attr(val: &str) -> bool {
    !matches!(val, "0" | "false" | "off")
}

/// Converts a twips integer to [`Points`].
///
/// 20 twips = 1 point (ECMA-376 §17.18.100).
#[must_use]
pub fn twips_to_points(twips: i32) -> Points {
    Points::new(f64::from(twips) / TWIPS_PER_PT)
}

/// Converts a half-points integer to [`Points`].
///
/// 2 half-points = 1 point (ECMA-376 §17.18.98). Used by `w:sz`/`w:szCs`.
#[must_use]
pub fn half_points_to_points(hp: i32) -> Points {
    Points::new(f64::from(hp) / HALF_POINTS_PER_PT)
}

/// Converts an EMU (English Metric Unit) integer to [`Points`].
///
/// 12 700 EMUs = 1 point; 914 400 EMUs = 1 inch (ECMA-376 §22.9.2.1).
#[must_use]
pub fn emu_to_points(emu: i64) -> Points {
    Points::new(emu as f64 / EMUS_PER_PT)
}

/// Parses a 6-character hexadecimal OOXML color string to an [`RgbColor`].
///
/// Returns `None` for the special value `"auto"`, the empty string, or any
/// string that is not exactly 6 valid hex digits. No `#` prefix is expected
/// (OOXML stores hex colors without it, e.g. `w:color w:val="FF0000"`).
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── bool_attr ─────────────────────────────────────────────────────────────

    #[test]
    fn bool_attr_true_values() {
        assert!(bool_attr("1"));
        assert!(bool_attr("true"));
        assert!(bool_attr("on"));
        // Unknown values also default to true per ECMA-376 §17.7.3
        assert!(bool_attr("yes"));
        assert!(bool_attr(""));
    }

    #[test]
    fn bool_attr_false_values() {
        assert!(!bool_attr("0"));
        assert!(!bool_attr("false"));
        assert!(!bool_attr("off"));
    }

    // ── twips_to_points ───────────────────────────────────────────────────────

    #[test]
    fn twips_to_points_zero() {
        assert_eq!(twips_to_points(0).value(), 0.0);
    }

    #[test]
    fn twips_to_points_one_pt() {
        assert_eq!(twips_to_points(20).value(), 1.0);
    }

    #[test]
    fn twips_to_points_720() {
        // 720 twips = 36 pt
        assert_eq!(twips_to_points(720).value(), 36.0);
    }

    #[test]
    fn twips_to_points_negative() {
        assert_eq!(twips_to_points(-20).value(), -1.0);
    }

    // ── half_points_to_points ─────────────────────────────────────────────────

    #[test]
    fn half_points_to_points_24() {
        // 24 half-points = 12 pt
        assert_eq!(half_points_to_points(24).value(), 12.0);
    }

    #[test]
    fn half_points_to_points_zero() {
        assert_eq!(half_points_to_points(0).value(), 0.0);
    }

    // ── emu_to_points ─────────────────────────────────────────────────────────

    #[test]
    fn emu_to_points_one_pt() {
        assert_eq!(emu_to_points(12700).value(), 1.0);
    }

    #[test]
    fn emu_to_points_one_inch() {
        // 914400 EMU = 72 pt
        assert!((emu_to_points(914_400).value() - 72.0).abs() < f64::EPSILON);
    }

    #[test]
    fn emu_to_points_zero() {
        assert_eq!(emu_to_points(0).value(), 0.0);
    }

    // ── hex_color ─────────────────────────────────────────────────────────────

    #[test]
    fn hex_color_red() {
        let c = hex_color("FF0000").unwrap();
        assert!((c.red() - 1.0_f32).abs() < 1e-6);
        assert!(c.green() < 1e-6);
        assert!(c.blue() < 1e-6);
    }

    #[test]
    fn hex_color_black() {
        let c = hex_color("000000").unwrap();
        assert!(c.red() < 1e-6);
        assert!(c.green() < 1e-6);
        assert!(c.blue() < 1e-6);
    }

    #[test]
    fn hex_color_white() {
        let c = hex_color("FFFFFF").unwrap();
        assert!((c.red() - 1.0_f32).abs() < 1e-6);
        assert!((c.green() - 1.0_f32).abs() < 1e-6);
        assert!((c.blue() - 1.0_f32).abs() < 1e-6);
    }

    #[test]
    fn hex_color_auto_is_none() {
        assert!(hex_color("auto").is_none());
    }

    #[test]
    fn hex_color_empty_is_none() {
        assert!(hex_color("").is_none());
    }

    #[test]
    fn hex_color_invalid_chars_is_none() {
        assert!(hex_color("GGGGGG").is_none());
    }

    #[test]
    fn hex_color_wrong_length_is_none() {
        assert!(hex_color("FFF").is_none());
        assert!(hex_color("FFFFFFF").is_none());
    }

    #[test]
    fn hex_color_lowercase() {
        // OOXML emits uppercase but parsers should tolerate lowercase
        let c = hex_color("ff8000").unwrap();
        assert!((c.red() - 1.0_f32).abs() < 1e-6);
        assert!((c.green() - 128.0_f32 / 255.0).abs() < 1e-4);
        assert!(c.blue() < 1e-6);
    }
}
