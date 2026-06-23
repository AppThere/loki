// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Writer for `docProps/custom.xml` (ECMA-376 §15.2.12.2) — carries the
//! extended Dublin Core fields that core.xml cannot represent, under reserved
//! `dcmi:` names (via [`DublinCoreMeta::to_named_pairs`]).

use loki_doc_model::meta::dublin_core::DublinCoreMeta;
use loki_opc::Package;
use loki_opc::part::{PartData, PartName};
use loki_opc::relationships::{Relationship, TargetMode};

use crate::error::OoxmlError;

/// Content type for the custom-properties part.
const MT_CUSTOM: &str = "application/vnd.openxmlformats-officedocument.custom-properties+xml";
/// OPC relationship type for the custom-properties part.
const REL_CUSTOM: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/custom-properties";
/// The fixed FMTID required on every `<property>` (ECMA-376 §22.7.2).
const FMTID: &str = "{D5CDD505-2E9C-101B-9397-08002B2CF9AE}";

/// Adds `docProps/custom.xml` (plus its package relationship and content-type
/// override) to `pkg` when `dc` carries any custom-property field. A no-op for
/// empty extended metadata.
pub(super) fn add_custom_properties(
    pkg: &mut Package,
    dc: &DublinCoreMeta,
) -> Result<(), OoxmlError> {
    // `dc:identifier` is written to core.xml as the native element, so drop it
    // from the custom properties to avoid duplicating it.
    let pairs: Vec<(String, String)> = dc
        .to_named_pairs()
        .into_iter()
        .filter(|(name, _)| name != "dcmi:identifier")
        .collect();
    if pairs.is_empty() {
        return Ok(());
    }

    let mut xml = String::from(concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
        "<Properties",
        " xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/custom-properties\"",
        " xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\">",
    ));
    // `pid` is a per-property id that must start at 2 and be unique (§22.7.2).
    for (pid, (name, value)) in pairs.iter().enumerate() {
        xml.push_str(&format!(
            "<property fmtid=\"{FMTID}\" pid=\"{pid}\" name=\"{}\"><vt:lpwstr>{}</vt:lpwstr></property>",
            escape(name),
            escape(value),
            pid = pid + 2,
        ));
    }
    xml.push_str("</Properties>");

    let part = PartName::new("/docProps/custom.xml").map_err(OoxmlError::Opc)?;
    pkg.set_part(part.clone(), PartData::new(xml.into_bytes(), MT_CUSTOM));
    pkg.relationships_mut()
        .add(Relationship {
            id: "rIdCustomProps".to_string(),
            rel_type: REL_CUSTOM.to_string(),
            target: "docProps/custom.xml".to_string(),
            target_mode: TargetMode::Internal,
        })
        .map_err(OoxmlError::Opc)?;
    pkg.content_type_map_mut().add_override(&part, MT_CUSTOM);
    Ok(())
}

/// Escapes XML text / attribute values.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}
