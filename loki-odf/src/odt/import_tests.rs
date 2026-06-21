// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for `import`.

use std::io::{Cursor, Write};

use zip::CompressionMethod;
use zip::write::{FileOptions, ZipWriter};

use super::*;
use crate::constants::{ENTRY_CONTENT, ENTRY_MANIFEST, ENTRY_STYLES, MIME_ODT};

fn build_odt_zip(version: Option<&str>) -> Vec<u8> {
    let ver_attr = match version {
        Some(v) => format!(" office:version=\"{v}\""),
        None => String::new(),
    };
    let content = format!(r#"<?xml version="1.0"?><office:document-content{ver_attr}/>"#);

    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));

    let stored = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
    zip.start_file("mimetype", stored).unwrap();
    zip.write_all(MIME_ODT.as_bytes()).unwrap();

    let deflated = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);
    zip.start_file(ENTRY_MANIFEST, deflated).unwrap();
    zip.write_all(b"<manifest:manifest/>").unwrap();
    zip.start_file(ENTRY_CONTENT, deflated).unwrap();
    zip.write_all(content.as_bytes()).unwrap();
    zip.start_file(ENTRY_STYLES, deflated).unwrap();
    zip.write_all(b"<office:document-styles/>").unwrap();

    zip.finish().unwrap();
    buf
}

#[test]
fn run_returns_source_version_1_2() {
    let zip = build_odt_zip(Some("1.2"));
    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .unwrap();
    assert_eq!(result.source_version, OdfVersion::V1_2);
    assert_eq!(
        result.document.source.as_ref().unwrap().version.as_deref(),
        Some("1.2")
    );
}

#[test]
fn run_absent_version_is_v1_1() {
    let zip = build_odt_zip(None);
    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .unwrap();
    assert_eq!(result.source_version, OdfVersion::V1_1);
}

#[test]
fn run_unknown_version_non_strict_emits_warning() {
    let zip = build_odt_zip(Some("99.0"));
    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .unwrap();
    assert_eq!(result.source_version, OdfVersion::V1_3);
    assert!(
        result.warnings.iter().any(|w| matches!(
            w,
            OdfWarning::UnrecognisedVersion { version }
                if version == "99.0"
        )),
        "expected UnrecognisedVersion warning"
    );
}

#[test]
fn run_unknown_version_strict_returns_error() {
    let zip = build_odt_zip(Some("99.0"));
    let opts = OdtImportOptions {
        strict_version: true,
        ..Default::default()
    };
    let result = OdtImporter::new(opts).run(Cursor::new(zip));
    assert!(
        matches!(
            result,
            Err(OdfError::UnsupportedVersion { ref version })
                if version == "99.0"
        ),
        "expected UnsupportedVersion error, got {result:?}"
    );
}
