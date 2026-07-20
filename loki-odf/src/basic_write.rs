// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `StarBasic` module **write-back** for the macro editor (macro spec §3.4).
//!
//! The ODF side of source-only write-back is far simpler than VBA: a module is
//! plain XML (`<script:module …>SOURCE</script:module>`), so editing means
//! replacing the element's text content. This rewrites a single module part,
//! preserving the XML declaration, any DOCTYPE, the element, and every attribute
//! verbatim — only the source text is swapped (and XML-escaped on the way out).
//! Nothing is executed; it is a byte transform, like the reader it inverts.

use std::collections::BTreeMap;

use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind};
use quick_xml::events::{BytesText, Event};
use quick_xml::{Reader, Writer};

use crate::basic::{is_module_file, local, parse_module};
use crate::error::{OdfError, OdfResult};

/// Rewrites one `<script:module>` XML file, replacing its source text with
/// `new_source` (which is XML-escaped as it is written). Everything else in the
/// file — declaration, DOCTYPE, element name, namespaces, attributes — is
/// preserved byte-for-byte.
///
/// # Errors
///
/// [`OdfError::Xml`] if the bytes are not well-formed XML, or
/// [`OdfError::MalformedElement`] if they contain no `<script:module>` element.
pub fn write_basic_module_source(module_xml: &[u8], new_source: &str) -> OdfResult<Vec<u8>> {
    let mut reader = Reader::from_reader(module_xml);
    reader.config_mut().trim_text(false);
    let mut writer = Writer::new(Vec::new());
    let mut buf = Vec::new();

    // Depth of `<script:module>` nesting (source text lives at depth ≥ 1 and is
    // dropped in favour of `new_source`, written once just after the open tag).
    let mut depth = 0u32;
    let mut found_module = false;

    loop {
        let event = reader
            .read_event_into(&mut buf)
            .map_err(|source| OdfError::Xml {
                part: MODULE_PART.into(),
                source,
            })?;

        let is_start = matches!(&event, Event::Start(e) if local(e.name().as_ref()) == b"module");
        let is_end = matches!(&event, Event::End(e) if local(e.name().as_ref()) == b"module");

        if matches!(&event, Event::Eof) {
            break;
        }

        if is_start {
            found_module = true;
            depth += 1;
            writer.write_event(event)?;
            if depth == 1 {
                writer.write_event(Event::Text(BytesText::new(new_source)))?;
            }
        } else if is_end {
            depth = depth.saturating_sub(1);
            writer.write_event(event)?;
        } else if depth >= 1 {
            // Inside the module: original content (text, escaped-entity refs,
            // CDATA) is being replaced by `new_source` — drop all of it.
        } else {
            writer.write_event(event)?;
        }
        buf.clear();
    }

    if !found_module || depth != 0 {
        return Err(OdfError::MalformedElement {
            element: "script:module".into(),
            part: MODULE_PART.into(),
            reason: "missing or unclosed <script:module> element".into(),
        });
    }
    Ok(writer.into_inner())
}

/// Applies edited source to every Basic module a payload names in `edits`
/// (keyed by module name, as [`crate::basic::extract_basic_modules`] reports it),
/// rewriting the affected parts in place. Returns how many modules were updated.
/// A non-`OdfBasic` payload is left untouched (returns 0).
///
/// # Errors
///
/// [`OdfError`] if a targeted module part is not well-formed XML.
pub fn apply_basic_edits(
    payload: &mut MacroPayload,
    edits: &BTreeMap<String, String>,
) -> OdfResult<usize> {
    if payload.kind != MacroPayloadKind::OdfBasic || edits.is_empty() {
        return Ok(0);
    }
    let mut rewrites: Vec<(String, Vec<u8>)> = Vec::new();
    for part in &payload.parts {
        if !is_module_file(&part.name) {
            continue;
        }
        let Some(module) = parse_module(&part.bytes, &part.name) else {
            continue;
        };
        if let Some(src) = edits.get(&module.name) {
            rewrites.push((
                part.name.clone(),
                write_basic_module_source(&part.bytes, src)?,
            ));
        }
    }
    let count = rewrites.len();
    for (name, bytes) in rewrites {
        payload.replace_part(&name, bytes);
    }
    Ok(count)
}

/// Placeholder part name for errors — the caller edits an in-memory module, so
/// there is no ZIP entry path at this layer.
const MODULE_PART: &str = "<script:module>";

#[cfg(test)]
#[path = "basic_write_tests.rs"]
mod tests;
