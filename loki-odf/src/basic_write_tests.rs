// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use std::collections::BTreeMap;

use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind, PreservedPart};

use super::{apply_basic_edits, write_basic_module_source};
use crate::basic::extract_basic_modules;

const MODULE: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<script:module xmlns:script="urn:oasis:names:tc:opendocument:xmlns:script:1.0" script:name="Module1" script:language="StarBasic">Sub Main
  MsgBox "hi"
End Sub</script:module>"#;

fn edits(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

/// Re-reading a rewritten module through the reader must yield the new source.
#[test]
fn rewrite_replaces_source_text() {
    let out = write_basic_module_source(MODULE, "Sub Other\n  Beep\nEnd Sub").unwrap();
    let payload = MacroPayload::new(
        MacroPayloadKind::OdfBasic,
        vec![PreservedPart::new(
            "Basic/Standard/Module1.xml",
            Some("text/xml".into()),
            out,
        )],
    );
    let mods = extract_basic_modules(&payload);
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0].name, "Module1", "name attribute preserved");
    assert_eq!(mods[0].source, "Sub Other\n  Beep\nEnd Sub");
}

/// The element, its attributes, and namespaces survive the rewrite.
#[test]
fn rewrite_preserves_attributes_and_declaration() {
    let out = write_basic_module_source(MODULE, "x").unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("script:name=\"Module1\""));
    assert!(text.contains("script:language=\"StarBasic\""));
    assert!(text.contains("xmlns:script="));
    assert!(text.starts_with("<?xml"));
}

/// Special characters in source are XML-escaped so the file stays well-formed.
#[test]
fn rewrite_escapes_special_characters() {
    let out = write_basic_module_source(MODULE, "If a < b And c > d & e Then").unwrap();
    let text = String::from_utf8(out.clone()).unwrap();
    assert!(text.contains("&lt;") && text.contains("&gt;") && text.contains("&amp;"));
    assert!(!text.contains("a < b"), "raw '<' would break the XML");

    // And it decodes back to the exact source.
    let payload = MacroPayload::new(
        MacroPayloadKind::OdfBasic,
        vec![PreservedPart::new("Basic/Standard/Module1.xml", None, out)],
    );
    assert_eq!(
        extract_basic_modules(&payload)[0].source,
        "If a < b And c > d & e Then"
    );
}

#[test]
fn non_module_xml_is_an_error() {
    assert!(write_basic_module_source(b"<other>x</other>", "y").is_err());
}

#[test]
fn malformed_xml_is_an_error() {
    assert!(write_basic_module_source(b"<script:module>unclosed", "y").is_err());
}

/// `apply_basic_edits` rewrites only the named module and leaves the rest.
#[test]
fn apply_edits_targets_named_module_only() {
    const MOD2: &[u8] = br#"<script:module xmlns:script="urn:oasis:names:tc:opendocument:xmlns:script:1.0" script:name="Helpers" script:language="StarBasic">Sub Keep
End Sub</script:module>"#;
    let mut payload = MacroPayload::new(
        MacroPayloadKind::OdfBasic,
        vec![
            PreservedPart::new(
                "Basic/Standard/Module1.xml",
                Some("text/xml".into()),
                MODULE.to_vec(),
            ),
            PreservedPart::new(
                "Basic/Standard/Helpers.xml",
                Some("text/xml".into()),
                MOD2.to_vec(),
            ),
        ],
    );

    let count =
        apply_basic_edits(&mut payload, &edits(&[("Module1", "Sub Changed\nEnd Sub")])).unwrap();
    assert_eq!(count, 1);

    let mods = extract_basic_modules(&payload);
    let src = |n: &str| mods.iter().find(|m| m.name == n).unwrap().source.clone();
    assert_eq!(src("Module1"), "Sub Changed\nEnd Sub");
    assert_eq!(
        src("Helpers"),
        "Sub Keep\nEnd Sub",
        "untargeted module untouched"
    );
}

#[test]
fn apply_edits_ignores_non_odf_and_empty() {
    let mut vba = MacroPayload::new(MacroPayloadKind::OoxmlVba, Vec::new());
    assert_eq!(
        apply_basic_edits(&mut vba, &edits(&[("A", "x")])).unwrap(),
        0
    );

    let mut basic = MacroPayload::new(
        MacroPayloadKind::OdfBasic,
        vec![PreservedPart::new(
            "Basic/Standard/Module1.xml",
            None,
            MODULE.to_vec(),
        )],
    );
    assert_eq!(apply_basic_edits(&mut basic, &edits(&[])).unwrap(), 0);
    // Unknown module name → nothing rewritten.
    assert_eq!(
        apply_basic_edits(&mut basic, &edits(&[("Nope", "x")])).unwrap(),
        0
    );
}
