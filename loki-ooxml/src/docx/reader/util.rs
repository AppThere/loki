// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Shared XML parsing utilities for OOXML event-mode readers.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::docx::model::styles::DocxCellMargins;
use crate::error::{OoxmlError, OoxmlResult};

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
            attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(std::borrow::Cow::into_owned)
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
pub fn toggle_prop(val_opt: Option<&str>) -> bool {
    match val_opt {
        Some("0" | "false" | "off") => false,
        _ => true, // absent, no val, "1"/"true"/"on", or unrecognised → true
    }
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
        assert!(toggle_prop(None));
    }

    #[test]
    fn toggle_zero_is_false() {
        assert!(!toggle_prop(Some("0")));
    }

    #[test]
    fn toggle_false_is_false() {
        assert!(!toggle_prop(Some("false")));
    }

    #[test]
    fn toggle_one_is_true() {
        assert!(toggle_prop(Some("1")));
    }
}

/// Parses a cell-margin container — `w:tcMar` (§17.4.68) on a cell, or
/// `w:tblCellMar` (§17.4.43) on a table/table style — into twips.
///
/// One parser for both because the two elements have identical content
/// (`w:top`/`w:bottom`/`w:left`/`w:right`, each `@w:w` in twips); only the
/// closing tag and the owning part differ. Keeping a second copy for
/// `tblCellMar` is how the two would drift — the `start`/`end` alias handling
/// below is exactly the sort of detail that gets fixed in one copy only.
///
/// Called after the container's Start event; consumes through the matching End.
/// COMPAT(ooxml-dxa): values are twentieths of a point; divide by 20 for points.
pub fn parse_cell_margins(
    reader: &mut Reader<&[u8]>,
    end_tag: &[u8],
    part: &str,
) -> OoxmlResult<DocxCellMargins> {
    let mut margins = DocxCellMargins::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                let twips: Option<i32> = attr_val(e, b"w").and_then(|v| v.parse().ok());
                match local_name(e.local_name().as_ref()) {
                    b"top" => margins.top = twips,
                    b"bottom" => margins.bottom = twips,
                    b"left" | b"start" => margins.left = twips,
                    b"right" | b"end" => margins.right = twips,
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if local_name(e.local_name().as_ref()) == end_tag => break,
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: part.into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(margins)
}
