// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! `StarBasic` module extraction from a preserved ODF script payload
//! (macro spec §9.6 — read-only viewing before any execution decision).
//!
//! ODF stores each Basic module as an XML file under `Basic/<library>/` with a
//! `<script:module script:name="…" script:language="StarBasic">SOURCE</…>`
//! element. This reads the module name and source text for the viewer; it never
//! executes anything. Library index files (`script-lb.xml`, `script-lc.xml`)
//! and dialogs are skipped.

use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind};
use quick_xml::Reader;
use quick_xml::events::Event;

/// A `StarBasic` module's extracted source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicModule {
    /// The module name (`script:name`, falling back to the file stem).
    pub name: String,
    /// The module source text.
    pub source: String,
}

/// Extracts every `StarBasic` module from a preserved ODF script payload.
/// Returns an empty list for a non-ODF payload.
#[must_use]
pub fn extract_basic_modules(payload: &MacroPayload) -> Vec<BasicModule> {
    if payload.kind != MacroPayloadKind::OdfBasic {
        return Vec::new();
    }
    let mut modules = Vec::new();
    for part in &payload.parts {
        if !is_module_file(&part.name) {
            continue;
        }
        if let Some(m) = parse_module(&part.bytes, &part.name) {
            modules.push(m);
        }
    }
    modules
}

/// Whether a preserved part path looks like a Basic *module* file (as opposed to
/// a library index or a directory entry).
fn is_module_file(path: &str) -> bool {
    let is_xml = std::path::Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xml"));
    let lower = path.to_ascii_lowercase();
    is_xml
        && !lower.ends_with("/script-lb.xml")
        && !lower.ends_with("/script-lc.xml")
        && !lower.contains("/dialog")
}

/// Parses a `<script:module>` file into a [`BasicModule`]. Returns `None` if the
/// file is not a module element.
fn parse_module(bytes: &[u8], path: &str) -> Option<BasicModule> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut in_module = false;
    let mut name: Option<String> = None;
    let mut source = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if local(e.name().as_ref()) == b"module" => {
                in_module = true;
                name = attr(e, b"name");
            }
            Ok(Event::Text(ref t)) if in_module => {
                if let Ok(text) = crate::xml_util::unescape_text(t) {
                    source.push_str(&text);
                }
            }
            Ok(Event::End(ref e)) if local(e.name().as_ref()) == b"module" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    if !in_module {
        return None;
    }
    Some(BasicModule {
        name: name.unwrap_or_else(|| file_stem(path)),
        source,
    })
}

fn local(qname: &[u8]) -> &[u8] {
    qname
        .iter()
        .rposition(|&b| b == b':')
        .map_or(qname, |p| &qname[p + 1..])
}

fn attr(e: &quick_xml::events::BytesStart<'_>, local_name: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if local(a.key.as_ref()) == local_name {
            String::from_utf8(a.value.into_owned()).ok()
        } else {
            None
        }
    })
}

fn file_stem(path: &str) -> String {
    path.rsplit('/')
        .next()
        .and_then(|f| f.strip_suffix(".xml"))
        .unwrap_or(path)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use loki_doc_model::io::macros::PreservedPart;

    fn payload(parts: Vec<PreservedPart>) -> MacroPayload {
        MacroPayload::new(MacroPayloadKind::OdfBasic, parts)
    }

    const MODULE: &[u8] = br#"<?xml version="1.0"?>
<script:module xmlns:script="urn:oasis:names:tc:opendocument:xmlns:script:1.0"
 script:name="Module1" script:language="StarBasic">Sub Main
  MsgBox "hi"
End Sub</script:module>"#;

    #[test]
    fn extracts_named_module_source() {
        let p = payload(vec![PreservedPart::new(
            "Basic/Standard/Module1.xml",
            Some("text/xml".into()),
            MODULE.to_vec(),
        )]);
        let mods = extract_basic_modules(&p);
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "Module1");
        assert!(mods[0].source.contains("MsgBox \"hi\""));
    }

    #[test]
    fn skips_library_index_and_directories() {
        let p = payload(vec![
            PreservedPart::new("Basic/", Some(String::new()), Vec::new()),
            PreservedPart::new(
                "Basic/script-lc.xml",
                Some("text/xml".into()),
                b"<x/>".to_vec(),
            ),
            PreservedPart::new(
                "Basic/Standard/script-lb.xml",
                Some("text/xml".into()),
                b"<x/>".to_vec(),
            ),
        ]);
        assert!(extract_basic_modules(&p).is_empty());
    }

    #[test]
    fn non_odf_payload_yields_nothing() {
        let p = MacroPayload::new(MacroPayloadKind::OoxmlVba, Vec::new());
        assert!(extract_basic_modules(&p).is_empty());
    }
}
