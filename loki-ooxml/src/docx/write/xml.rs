// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Low-level XML writing helpers for DOCX serialization.

use quick_xml::Writer;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use std::io::Write;

// ── OOXML namespace URIs ─────────────────────────────────────────────────────

pub(super) const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
pub(super) const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub(super) const NS_WP: &str =
    "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
pub(super) const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub(super) const NS_PIC: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
pub(super) const NS_WPS: &str = "http://schemas.microsoft.com/office/word/2010/wordprocessingShape";

pub(super) const REL_HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
pub(super) const REL_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
pub(super) const REL_STYLES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
pub(super) const REL_NUMBERING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
pub(super) const REL_FOOTNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes";
pub(super) const REL_ENDNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes";
pub(super) const REL_HEADER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
pub(super) const REL_FOOTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";

// ── Unit conversions ─────────────────────────────────────────────────────────

/// Points → twips (1 pt = 20 twips). Used for page size, margins, table widths.
#[allow(clippy::cast_possible_truncation)]
pub(super) fn pts_to_twips(pt: f64) -> i32 {
    (pt * 20.0).round() as i32
}

/// Points → half-points (1 pt = 2 half-pts). Used for `w:sz`.
#[allow(clippy::cast_possible_truncation)]
pub(super) fn pts_to_half_pts(pt: f64) -> i32 {
    (pt * 2.0).round() as i32
}

// ── Color formatting ─────────────────────────────────────────────────────────

/// Converts an RRGGBB hex string (with or without `#`) to the 6-char
/// uppercase form required by OOXML `w:color/@w:val`.
pub(super) fn hex_color_val(hex: &str) -> String {
    hex.trim_start_matches('#').to_uppercase()
}

pub(super) fn color_to_hex(color: &loki_primitives::color::DocumentColor) -> String {
    color.to_hex().map_or_else(
        || "000000".to_string(),
        |s| s.trim_start_matches('#').to_string(),
    )
}

// ── Writer helpers ───────────────────────────────────────────────────────────

/// Writes the `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>` declaration.
pub(super) fn write_decl<W: Write>(w: &mut Writer<W>) -> quick_xml::Result<()> {
    Ok(w.write_event(Event::Decl(BytesDecl::new(
        "1.0",
        Some("UTF-8"),
        Some("yes"),
    )))?)
}

/// Writes a self-closing element: `<tag attr1="v1" attr2="v2"/>`.
/// `attrs` is a slice of `(local_name, value)` pairs — caller is responsible
/// for prefixes (e.g. `"w:val"`).
pub(super) fn write_empty<W: Write>(
    w: &mut Writer<W>,
    tag: &str,
    attrs: &[(&str, &str)],
) -> quick_xml::Result<()> {
    let mut e = BytesStart::new(tag);
    for (k, v) in attrs {
        e.push_attribute((*k, *v));
    }
    Ok(w.write_event(Event::Empty(e))?)
}

/// Writes `<tag attrs>text content</tag>`.
pub(super) fn write_text_elem<W: Write>(
    w: &mut Writer<W>,
    tag: &str,
    attrs: &[(&str, &str)],
    text: &str,
) -> quick_xml::Result<()> {
    let mut start = BytesStart::new(tag);
    for (k, v) in attrs {
        start.push_attribute((*k, *v));
    }
    w.write_event(Event::Start(start))?;
    w.write_event(Event::Text(BytesText::new(text)))?;
    Ok(w.write_event(Event::End(BytesEnd::new(tag)))?)
}

/// Writes `<tag attrs>` (opening tag only).
pub(super) fn write_start<W: Write>(
    w: &mut Writer<W>,
    tag: &str,
    attrs: &[(&str, &str)],
) -> quick_xml::Result<()> {
    let mut e = BytesStart::new(tag);
    for (k, v) in attrs {
        e.push_attribute((*k, *v));
    }
    Ok(w.write_event(Event::Start(e))?)
}

/// Writes `</tag>` (closing tag).
pub(super) fn write_end<W: Write>(w: &mut Writer<W>, tag: &str) -> quick_xml::Result<()> {
    Ok(w.write_event(Event::End(BytesEnd::new(tag)))?)
}

/// Shorthand for a single `w:val="..."` attribute slice.
/// Usage: `write_empty(w, "w:pStyle", &wval(&style_id))?;`
pub(super) fn wval(v: &str) -> [(&str, &str); 1] {
    [("w:val", v)]
}
