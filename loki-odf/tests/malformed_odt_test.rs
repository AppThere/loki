// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Regression tests verifying that malformed ODT input returns `Err` rather
//! than panicking.
//!
//! Each test feeds a structurally invalid package or XML document to
//! [`OdtImporter::run`] and asserts the result is `Err(_)`.  If any
//! production `unwrap()` is accidentally reintroduced in the reader, the
//! corresponding test will panic in CI rather than silently pass.

mod helpers;

use std::io::Cursor;

use loki_odf::odt::import::{OdtImportOptions, OdtImporter};

/// A completely empty byte slice is not a valid ZIP — must return `Err`.
#[test]
fn empty_bytes_returns_error() {
    let result = OdtImporter::new(OdtImportOptions::default()).run(Cursor::new(b""));
    assert!(
        result.is_err(),
        "empty input must return Err, not Ok or panic"
    );
}

/// Random non-ZIP bytes are not a valid package — must return `Err`.
#[test]
fn garbage_bytes_returns_error() {
    let garbage = b"this is not a ZIP file at all \x00\x01\x02\xFF";
    let result =
        OdtImporter::new(OdtImportOptions::default()).run(Cursor::new(garbage.as_slice()));
    assert!(
        result.is_err(),
        "garbage bytes must return Err, not Ok or panic"
    );
}

/// A valid ZIP but missing the mandatory `content.xml` entry — must return `Err`.
#[test]
fn zip_missing_content_xml_returns_error() {
    // Build an ODT ZIP that has only `mimetype` and `styles.xml` — no `content.xml`.
    let styles = helpers::empty_styles_xml("1.2");
    let zip = helpers::build_odt_zip_no_content(&styles);
    let result = OdtImporter::new(OdtImportOptions::default()).run(Cursor::new(zip));
    assert!(
        result.is_err(),
        "ZIP without content.xml must return Err, not Ok or panic"
    );
}

/// A valid ZIP whose `content.xml` is truncated mid-element — must return `Err`.
#[test]
fn truncated_content_xml_returns_error() {
    let truncated_content = b"<?xml version=\"1.0\"?><office:document-content";
    // No closing tag — parser will hit EOF mid-element.
    let styles = helpers::empty_styles_xml("1.2");
    let zip = helpers::build_odt_zip(truncated_content, &styles, None);
    let result = OdtImporter::new(OdtImportOptions::default()).run(Cursor::new(zip));
    assert!(
        result.is_err(),
        "truncated content.xml must return Err, not Ok or panic"
    );
}

/// A valid ZIP whose `content.xml` contains a missing required child of
/// `office:body` (empty `office:text` is still valid; so we omit the namespace
/// declaration to force an XML-level error).
#[test]
fn content_xml_invalid_xml_returns_error() {
    // Invalid XML: closing tag name does not match opening tag.
    let bad_xml =
        b"<?xml version=\"1.0\"?><office:document-content></office:document-WRONG>";
    let styles = helpers::empty_styles_xml("1.2");
    let zip = helpers::build_odt_zip(bad_xml, &styles, None);
    let result = OdtImporter::new(OdtImportOptions::default()).run(Cursor::new(zip));
    assert!(
        result.is_err(),
        "mismatched XML tags must return Err, not Ok or panic"
    );
}
