// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Reader for `docProps/custom.xml` (ECMA-376 §15.2.12.2): extracts the
//! `(name, value)` custom-property pairs so the extended Dublin Core fields can
//! be reconstructed via [`DublinCoreMeta::from_named_pairs`].

use loki_doc_model::meta::dublin_core::DublinCoreMeta;
use loki_opc::{Package, PartName};
use quick_xml::Reader;
use quick_xml::events::Event;

use crate::docx::reader::util::{attr_val, local_name};
use crate::error::{OoxmlError, OoxmlResult};
use crate::xml_util::{resolve_general_ref, unescape_text};

/// Reads `docProps/custom.xml` (resolved via the package custom-properties
/// relationship) and merges its reserved `dcmi:` fields into `dc`, preserving
/// any `identifier` already mapped from core.xml. A no-op when the part is
/// absent or unparsable.
pub(crate) fn apply_extended_dc(package: &Package, dc: &mut DublinCoreMeta) {
    // Match both transitional and strict relationship namespaces by suffix.
    let Some(rel) = package
        .relationships()
        .iter()
        .find(|r| r.rel_type.ends_with("custom-properties"))
    else {
        return;
    };
    let target = if rel.target.starts_with('/') {
        rel.target.clone()
    } else {
        format!("/{}", rel.target)
    };
    let Ok(part_name) = PartName::new(&target) else {
        return;
    };
    let Some(part) = package.part(&part_name) else {
        return;
    };
    let Ok(pairs) = parse_custom_props(&part.bytes) else {
        return;
    };
    let identifier = dc.identifier.take();
    *dc = DublinCoreMeta::from_named_pairs(&pairs);
    dc.identifier = identifier;
}

/// Parses `docProps/custom.xml` into `(name, value)` pairs. Only the string
/// (`vt:lpwstr`) value type is read; other typed values are skipped.
pub(crate) fn parse_custom_props(xml: &[u8]) -> OoxmlResult<Vec<(String, String)>> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut pairs = Vec::new();

    // The `name` of the `<property>` currently open, and the text accumulated
    // from its `<vt:lpwstr>` child.
    let mut current_name: Option<String> = None;
    let mut in_value = false;
    let mut value = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match local_name(e.local_name().as_ref()) {
                b"property" => {
                    current_name = attr_val(e, b"name");
                    value.clear();
                }
                b"lpwstr" => in_value = current_name.is_some(),
                _ => {}
            },
            Ok(Event::Text(ref t)) if in_value => {
                if let Ok(s) = unescape_text(t) {
                    value.push_str(&s);
                }
            }
            Ok(Event::GeneralRef(ref r)) if in_value => {
                if let Ok(s) = resolve_general_ref(r) {
                    value.push_str(&s);
                }
            }
            Ok(Event::End(ref e)) => match local_name(e.local_name().as_ref()) {
                b"lpwstr" => in_value = false,
                b"property" => {
                    if let Some(name) = current_name.take() {
                        pairs.push((name, std::mem::take(&mut value)));
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(OoxmlError::Xml {
                    part: "docProps/custom.xml".into(),
                    source: e,
                });
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(pairs)
}
