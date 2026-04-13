// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Shared XML parsing utilities for OOXML event-mode readers.

use quick_xml::events::BytesStart;

/// Returns the local name (without namespace prefix) from an element's bytes.
///
/// OOXML uses many namespace prefixes (`w:`, `wp:`, `a:`, etc.). We match
/// on local names only. ECMA-376 §L.5.
#[must_use]
pub fn local_name(bytes: &[u8]) -> &[u8] {
    if let Some(pos) = bytes.iter().position(|&b| b == b':') {
        &bytes[pos + 1..]
    } else {
        bytes
    }
}

/// Extracts the value of an attribute by its local name (without prefix).
///
/// Returns `None` if the attribute is not present.
#[must_use]
pub fn attr_val<'a>(start: &'a BytesStart<'a>, local: &[u8]) -> Option<String> {
    start.attributes().flatten().find_map(|attr| {
        if local_name(attr.key.as_ref()) == local {
            attr.unescape_value().ok().map(|v| v.into_owned())
        } else {
            None
        }
    })
}

/// Parses a toggle property (`w:b`, `w:i`, etc.) attribute value.
///
/// Toggle properties follow ECMA-376 §17.7.3: the element being present
/// with no `@w:val` or `@w:val="1"/"true"/"on"` means `true`; `@w:val="0"/
/// "false"/"off"` means `false`. Absent element means `None` (inherit).
#[must_use]
pub fn toggle_prop(val_opt: Option<&str>) -> Option<bool> {
    match val_opt {
        None => Some(true), // present, no val → true
        Some("1") | Some("true") | Some("on") => Some(true),
        Some("0") | Some("false") | Some("off") => Some(false),
        _ => Some(true), // unrecognised → true
    }
}

/// Parses an integer twips value from an attribute string.
///
/// Returns `None` if the attribute is absent or not a valid integer.
#[must_use]
pub fn parse_twips(s: &str) -> Option<i32> {
    s.parse::<i32>().ok()
}

/// Parses an i64 EMU value from an attribute string.
#[must_use]
pub fn parse_emu(s: &str) -> Option<i64> {
    s.parse::<i64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_absent_is_true() {
        assert_eq!(toggle_prop(None), Some(true));
    }

    #[test]
    fn toggle_zero_is_false() {
        assert_eq!(toggle_prop(Some("0")), Some(false));
    }

    #[test]
    fn toggle_false_is_false() {
        assert_eq!(toggle_prop(Some("false")), Some(false));
    }

    #[test]
    fn toggle_one_is_true() {
        assert_eq!(toggle_prop(Some("1")), Some(true));
    }

    #[test]
    fn twips_720_to_i32() {
        assert_eq!(parse_twips("720"), Some(720));
    }
}
