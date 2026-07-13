// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Unit tests for the ODT package reader (`super`). Extracted from package.rs (Phase 7.1 inline-test extraction).

use std::io::{Cursor, Write};

use zip::CompressionMethod;
use zip::write::{FileOptions, ZipWriter};

use super::*;
use crate::constants::{ENTRY_MIMETYPE, MIME_ODT};
use crate::version::OdfVersion;

/// Build a minimal in-memory ODF ZIP with the given entries.
///
/// `extra_entries` is a list of `(name, content, compressed)` tuples.
fn build_zip(
    mimetype_first: bool,
    mimetype_content: &[u8],
    extra_entries: &[(&str, &[u8])],
) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));

    if mimetype_first {
        let opts = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
        zip.start_file(ENTRY_MIMETYPE, opts).unwrap();
        zip.write_all(mimetype_content).unwrap();
    }

    for (name, data) in extra_entries {
        let opts = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);
        zip.start_file(*name, opts).unwrap();
        zip.write_all(data).unwrap();
    }

    if !mimetype_first {
        let opts = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
        zip.start_file(ENTRY_MIMETYPE, opts).unwrap();
        zip.write_all(mimetype_content).unwrap();
    }

    zip.finish().unwrap();
    buf
}

/// Minimal content.xml with a given version attribute (or absent).
fn content_xml(version: Option<&str>) -> Vec<u8> {
    let ver_attr = match version {
        Some(v) => format!(" office:version=\"{v}\""),
        None => String::new(),
    };
    format!(r#"<?xml version="1.0"?><office:document-content{ver_attr}/>"#).into_bytes()
}

fn minimal_zip(version: Option<&str>) -> Vec<u8> {
    let manifest = b"<manifest:manifest/>";
    let content = content_xml(version);
    build_zip(
        true,
        MIME_ODT.as_bytes(),
        &[
            (ENTRY_MANIFEST, manifest),
            (ENTRY_CONTENT, &content),
            (ENTRY_STYLES, b"<office:document-styles/>"),
        ],
    )
}

// ── open succeeds for well-formed package ─────────────────────────────

#[test]
fn open_minimal_package_succeeds() {
    let zip_bytes = minimal_zip(Some("1.3"));
    let result = OdfPackage::open(Cursor::new(zip_bytes));
    assert!(result.is_ok(), "Expected Ok, got {result:?}");
}

// ── mimetype must be first entry ──────────────────────────────────────

#[test]
fn open_mimetype_not_first_fails() {
    let content = content_xml(Some("1.2"));
    let zip_bytes = build_zip(
        false, // mimetype is NOT first
        MIME_ODT.as_bytes(),
        &[
            (ENTRY_MANIFEST, b"<manifest:manifest/>"),
            (ENTRY_CONTENT, &content),
        ],
    );
    let result = OdfPackage::open(Cursor::new(zip_bytes));
    assert!(
        matches!(result, Err(OdfError::MalformedElement { .. })),
        "Expected MalformedElement, got {result:?}"
    );
}

// ── mimetype with trailing newline fails ──────────────────────────────

#[test]
fn open_mimetype_trailing_newline_fails() {
    let mut mime_with_nl = MIME_ODT.as_bytes().to_vec();
    mime_with_nl.push(b'\n');

    let content = content_xml(Some("1.3"));
    let zip_bytes = build_zip(
        true,
        &mime_with_nl,
        &[
            (ENTRY_MANIFEST, b"<manifest:manifest/>"),
            (ENTRY_CONTENT, &content),
        ],
    );
    let result = OdfPackage::open(Cursor::new(zip_bytes));
    assert!(
        matches!(result, Err(OdfError::MalformedElement { .. })),
        "Expected MalformedElement, got {result:?}"
    );
}

// ── missing content.xml fails ─────────────────────────────────────────

#[test]
fn open_missing_content_xml_fails() {
    let zip_bytes = build_zip(
        true,
        MIME_ODT.as_bytes(),
        &[(ENTRY_MANIFEST, b"<manifest:manifest/>")],
    );
    let result = OdfPackage::open(Cursor::new(zip_bytes));
    assert!(
        matches!(result, Err(OdfError::MissingPart { ref part }) if part == ENTRY_CONTENT),
        "Expected MissingPart(content.xml), got {result:?}"
    );
}

// ── detect_version: office:version="1.2" ─────────────────────────────

#[test]
fn detect_version_1_2() {
    let content = content_xml(Some("1.2"));
    let (v, absent) = OdfPackage::detect_version(&content).unwrap();
    assert_eq!(v, OdfVersion::V1_2);
    assert!(!absent);
}

// ── detect_version: absent → V1_1, version_was_absent=true ───────────

#[test]
fn detect_version_absent_is_v1_1() {
    let content = content_xml(None);
    let (v, absent) = OdfPackage::detect_version(&content).unwrap();
    assert_eq!(v, OdfVersion::V1_1);
    assert!(absent);
}

// ── detect_version: unrecognised → V1_3, version_was_absent=false ────

#[test]
fn detect_version_unknown_falls_back_to_v1_3() {
    let content = content_xml(Some("99.0"));
    let (v, absent) = OdfPackage::detect_version(&content).unwrap();
    assert_eq!(v, OdfVersion::V1_3);
    assert!(!absent);
}

fn encode_utf16(s: &str, be: bool) -> Vec<u8> {
    let u16s: Vec<u16> = s.encode_utf16().collect();
    let mut bytes = Vec::new();
    if be {
        bytes.push(0xFE);
        bytes.push(0xFF);
        for val in u16s {
            bytes.extend_from_slice(&val.to_be_bytes());
        }
    } else {
        bytes.push(0xFF);
        bytes.push(0xFE);
        for val in u16s {
            bytes.extend_from_slice(&val.to_le_bytes());
        }
    }
    bytes
}

#[test]
fn test_open_utf16_package_transcodes_to_utf8() {
    let content_str = r#"<?xml version="1.0"?><office:document-content office:version="1.3"/>"#;
    let content_be = encode_utf16(content_str, true);

    let styles_str = r#"<?xml version="1.0"?><office:document-styles/>"#;
    let styles_le = encode_utf16(styles_str, false);

    let zip_bytes = build_zip(
        true,
        MIME_ODT.as_bytes(),
        &[
            (ENTRY_MANIFEST, b"<manifest:manifest/>"),
            (ENTRY_CONTENT, &content_be),
            (ENTRY_STYLES, &styles_le),
        ],
    );

    let pkg = OdfPackage::open(Cursor::new(zip_bytes)).unwrap();

    let content_utf8 = String::from_utf8(pkg.content).unwrap();
    let styles_utf8 = String::from_utf8(pkg.styles).unwrap();

    assert_eq!(content_utf8, content_str);
    assert_eq!(styles_utf8, styles_str);
    assert_eq!(pkg.version, OdfVersion::V1_3);
}
