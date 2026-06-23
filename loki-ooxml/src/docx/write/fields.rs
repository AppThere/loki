// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Writer for OOXML complex document fields (`w:fldChar` / `w:instrText`).
//!
//! A [`Field`] is serialized as the standard complex-field run sequence
//! (ECMA-376 §17.16.18):
//! `begin → instrText → [separate → result] → end`. We emit the complex
//! form (rather than `w:fldSimple`) because the importer's field state
//! machine only reads complex fields, so this is what round-trips.

use quick_xml::Writer;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use std::io::Write;

use loki_doc_model::content::field::types::{CrossRefFormat, Field, FieldKind};

use super::document::{RunProps, write_text_run};
use super::xml::{write_empty, write_end, write_start};

/// Serializes a [`Field`] as a complex field, applying `props` to the
/// displayed result run so inline formatting (bold, style, …) is preserved.
pub(super) fn write_field<W: Write>(w: &mut Writer<W>, field: &Field, props: &RunProps) {
    let instruction = field_instruction(&field.kind);
    // A future `FieldKind` we cannot map yields an empty instruction; emit
    // nothing rather than a malformed field. All current variants map.
    if instruction.is_empty() {
        return;
    }

    write_fld_char(w, "begin");
    write_instr_text(w, &format!(" {instruction} "));

    // The result snapshot is optional: emit `separate` + the cached value only
    // when one is present, so a field imported without a snapshot round-trips
    // back to `current_value == None`.
    if let Some(value) = &field.current_value {
        write_fld_char(w, "separate");
        write_text_run(w, value, props);
    }

    write_fld_char(w, "end");
}

/// Writes `<w:r><w:fldChar w:fldCharType="TYPE"/></w:r>`.
fn write_fld_char<W: Write>(w: &mut Writer<W>, kind: &str) {
    let _ = write_start(w, "w:r", &[]);
    let _ = write_empty(w, "w:fldChar", &[("w:fldCharType", kind)]);
    let _ = write_end(w, "w:r");
}

/// Writes `<w:r><w:instrText xml:space="preserve">TEXT</w:instrText></w:r>`.
fn write_instr_text<W: Write>(w: &mut Writer<W>, text: &str) {
    let _ = write_start(w, "w:r", &[]);
    let mut start = BytesStart::new("w:instrText");
    start.push_attribute(("xml:space", "preserve"));
    let _ = w.write_event(Event::Start(start));
    let _ = w.write_event(Event::Text(BytesText::new(text)));
    let _ = w.write_event(Event::End(BytesEnd::new("w:instrText")));
    let _ = write_end(w, "w:r");
}

/// Builds the OOXML field instruction string for a [`FieldKind`].
///
/// This is the inverse of the importer's `parse_field_instruction`, so a
/// known field round-trips to the same [`FieldKind`].
fn field_instruction(kind: &FieldKind) -> String {
    match kind {
        FieldKind::PageNumber => "PAGE".to_string(),
        FieldKind::PageCount => "NUMPAGES".to_string(),
        FieldKind::Date { format } => date_like("DATE", format.as_deref()),
        FieldKind::Time { format } => date_like("TIME", format.as_deref()),
        FieldKind::Title => "TITLE".to_string(),
        FieldKind::Author => "AUTHOR".to_string(),
        FieldKind::Subject => "SUBJECT".to_string(),
        FieldKind::FileName => "FILENAME".to_string(),
        FieldKind::WordCount => "NUMWORDS".to_string(),
        FieldKind::CrossReference { target, format } => match format {
            // PAGEREF re-imports as `CrossRefFormat::Page`; everything else
            // maps to a plain `REF` (which re-imports as `Number`).
            CrossRefFormat::Page => format!("PAGEREF {target}"),
            _ => format!("REF {target}"),
        },
        FieldKind::Raw { instruction } => instruction.clone(),
        // `FieldKind` is `#[non_exhaustive]`; an unknown future variant has no
        // known instruction string and is skipped by the caller.
        _ => String::new(),
    }
}

/// Formats a `DATE`/`TIME` field, appending a `\@ "format"` switch when set.
fn date_like(name: &str, format: Option<&str>) -> String {
    match format {
        Some(f) => format!("{name} \\@ \"{f}\""),
        None => name.to_string(),
    }
}

#[cfg(test)]
#[path = "fields_tests.rs"]
mod tests;
