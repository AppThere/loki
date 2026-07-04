// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

use std::path::Path;

use super::super::{SchemaKind, SchemaValidator};
use super::XmllintValidator;

const NOTE_XSD: &str = r#"<?xml version="1.0"?>
<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
  <xs:element name="note" type="xs:string"/>
</xs:schema>
"#;

const NOTE_RNG: &str = r#"<?xml version="1.0"?>
<element name="note" xmlns="http://relaxng.org/ns/structure/1.0">
  <text/>
</element>
"#;

/// Writes `content` to `dir/name` and returns the path.
fn write(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, content).expect("write fixture");
    p
}

/// Skips the test (returning the validator) only when `xmllint` is present;
/// otherwise prints a notice and returns `None`.
fn validator_or_skip(test: &str) -> Option<XmllintValidator> {
    match XmllintValidator::new() {
        Ok(v) => Some(v),
        Err(_) => {
            eprintln!("skip {test}: xmllint not available");
            None
        }
    }
}

#[test]
fn missing_binary_fails_loudly() {
    // No xmllint needed: an absent binary must error, never silently pass.
    let err = XmllintValidator::with_program("appthere-conformance-no-such-xmllint")
        .expect_err("absent binary must error");
    assert!(
        matches!(err, super::super::SchemaError::XmllintNotFound),
        "expected XmllintNotFound, got {err:?}"
    );
}

#[test]
fn is_available_is_callable() {
    // Exercises the detection path; the value depends on the environment.
    let _ = XmllintValidator::is_available();
}

#[test]
fn valid_document_passes_xsd() {
    let Some(v) = validator_or_skip("valid_document_passes_xsd") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let xsd = write(dir.path(), "note.xsd", NOTE_XSD);
    let xml = write(dir.path(), "good.xml", "<note>hello</note>\n");
    let report = v.validate_file(&xml, &xsd, SchemaKind::Xsd).unwrap();
    assert!(report.valid, "expected valid, got {report:?}");
    assert!(report.violations.is_empty());
}

#[test]
fn invalid_document_fails_with_located_violation() {
    let Some(v) = validator_or_skip("invalid_document_fails_with_located_violation") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let xsd = write(dir.path(), "note.xsd", NOTE_XSD);
    let xml = write(dir.path(), "bad.xml", "<wrong>hello</wrong>\n");
    let report = v.validate_file(&xml, &xsd, SchemaKind::Xsd).unwrap();
    assert!(!report.valid, "expected invalid");
    assert!(
        !report.violations.is_empty(),
        "expected at least one violation"
    );
    // xmllint locates the offending element on line 1.
    assert_eq!(report.violations[0].line, Some(1));
    assert!(
        report.violations[0]
            .message
            .to_lowercase()
            .contains("wrong")
    );
}

#[test]
fn valid_document_passes_relaxng() {
    let Some(v) = validator_or_skip("valid_document_passes_relaxng") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let rng = write(dir.path(), "note.rng", NOTE_RNG);
    let xml = write(dir.path(), "good.xml", "<note>hello</note>\n");
    let report = v.validate_file(&xml, &rng, SchemaKind::RelaxNg).unwrap();
    assert!(report.valid, "expected valid, got {report:?}");
}

#[test]
fn validate_bytes_writes_and_validates() {
    let Some(v) = validator_or_skip("validate_bytes_writes_and_validates") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let xsd = write(dir.path(), "note.xsd", NOTE_XSD);
    let good = v
        .validate_bytes(b"<note>hi</note>", &xsd, SchemaKind::Xsd)
        .unwrap();
    assert!(good.valid);
    let bad = v.validate_bytes(b"<nope/>", &xsd, SchemaKind::Xsd).unwrap();
    assert!(!bad.valid);
}
