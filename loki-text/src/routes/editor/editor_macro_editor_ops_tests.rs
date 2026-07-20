// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use std::collections::BTreeMap;

use loki_doc_model::io::macros::{MacroPayload, MacroPayloadKind, PreservedPart};

use super::{build_edited_payload, changed_edits};

const MODULE: &[u8] = br#"<script:module xmlns:script="urn:oasis:names:tc:opendocument:xmlns:script:1.0" script:name="Module1" script:language="StarBasic">Sub Main
End Sub</script:module>"#;

fn odf_payload() -> MacroPayload {
    MacroPayload::new(
        MacroPayloadKind::OdfBasic,
        vec![PreservedPart::new(
            "Basic/Standard/Module1.xml",
            Some("text/xml".into()),
            MODULE.to_vec(),
        )],
    )
}

fn edits(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

#[test]
fn odf_edit_rewrites_module_and_changes_hash() {
    let original = odf_payload();
    let before = original.payload_hash();

    let edited = build_edited_payload(
        &original,
        &edits(&[("Module1", "Sub Main\n  Beep\nEnd Sub")]),
    )
    .expect("ODF edit applies");

    assert_ne!(
        edited.payload_hash(),
        before,
        "edited content re-keys the hash"
    );
    let mods = loki_odf::basic::extract_basic_modules(&edited);
    assert_eq!(mods[0].source, "Sub Main\n  Beep\nEnd Sub");
}

#[test]
fn odf_edit_of_unknown_module_is_a_noop_payload() {
    let original = odf_payload();
    let edited =
        build_edited_payload(&original, &edits(&[("Ghost", "x")])).expect("no-op edit is ok");
    assert_eq!(
        edited.payload_hash(),
        original.payload_hash(),
        "editing a non-existent module changes nothing"
    );
}

#[test]
fn vba_payload_without_project_part_errors() {
    // A VBA-kind payload missing its vbaProject.bin cannot be rewritten.
    let p = MacroPayload::new(
        MacroPayloadKind::OoxmlVba,
        vec![PreservedPart::new(
            "/word/vbaData.xml",
            None,
            b"<x/>".to_vec(),
        )],
    );
    assert!(build_edited_payload(&p, &edits(&[("M", "y")])).is_err());
}

#[test]
fn changed_edits_reports_only_differences() {
    let names = vec!["A".to_string(), "B".to_string(), "C".to_string()];
    let originals = vec!["one".to_string(), "two".to_string(), "three".to_string()];
    let drafts = vec!["one".to_string(), "TWO".to_string(), "three".to_string()];

    let edits = changed_edits(&names, &originals, &drafts);
    assert_eq!(edits.len(), 1);
    assert_eq!(edits.get("B").map(String::as_str), Some("TWO"));
}

#[test]
fn changed_edits_empty_when_nothing_changed() {
    let names = vec!["A".to_string()];
    let same = vec!["x".to_string()];
    assert!(changed_edits(&names, &same, &same).is_empty());
}
