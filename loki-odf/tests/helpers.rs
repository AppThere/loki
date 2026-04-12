// Copyright 2024-2026 AppThere
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Minimal ODT package builder for integration tests.
//!
//! [`build_odt_zip`] produces a valid ODF ZIP archive from raw XML byte
//! slices. The helper XML constructors (`heading_and_paragraphs_content_xml`,
//! etc.) produce well-formed `content.xml` / `styles.xml` fixtures.

use std::io::{Cursor, Write};
use zip::write::{FileOptions, ZipWriter};
use zip::CompressionMethod;

/// The ODF text MIME type, stored verbatim (no trailing newline).
pub const MIME_ODT: &str = "application/vnd.oasis.opendocument.text";

/// A minimal `META-INF/manifest.xml` that satisfies the package validator.
pub const MANIFEST: &[u8] = b"<manifest:manifest \
    xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\" \
    manifest:version=\"1.2\"/>";

/// Build an in-memory ODF ZIP archive.
///
/// `content_xml` and `styles_xml` are written as-is. If `meta_xml` is
/// `Some`, a `meta.xml` entry is included.
///
/// The `mimetype` entry is always first and uncompressed (stored), in
/// compliance with ODF 1.3 §3.4.
pub fn build_odt_zip(
    content_xml: &[u8],
    styles_xml: &[u8],
    meta_xml: Option<&[u8]>,
) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));

    // mimetype must be first and stored (uncompressed)
    let stored = FileOptions::<()>::default()
        .compression_method(CompressionMethod::Stored);
    zip.start_file("mimetype", stored).unwrap();
    zip.write_all(MIME_ODT.as_bytes()).unwrap();

    let deflated = FileOptions::<()>::default()
        .compression_method(CompressionMethod::Deflated);

    zip.start_file("META-INF/manifest.xml", deflated).unwrap();
    zip.write_all(MANIFEST).unwrap();

    zip.start_file("content.xml", deflated).unwrap();
    zip.write_all(content_xml).unwrap();

    zip.start_file("styles.xml", deflated).unwrap();
    zip.write_all(styles_xml).unwrap();

    if let Some(meta) = meta_xml {
        zip.start_file("meta.xml", deflated).unwrap();
        zip.write_all(meta).unwrap();
    }

    zip.finish().unwrap();
    buf
}

/// Minimal `styles.xml` with the given version attribute.
///
/// Omit the version attribute (ODF 1.1 style) by passing an empty string or
/// constructing raw XML by hand.
pub fn empty_styles_xml(version: &str) -> Vec<u8> {
    let ver_attr = if version.is_empty() {
        String::new()
    } else {
        format!(" office:version=\"{version}\"")
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <office:document-styles{ver_attr} \
         xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\">\
         <office:styles/>\
         <office:automatic-styles/>\
         <office:master-styles/>\
         </office:document-styles>"
    )
    .into_bytes()
}

/// `content.xml` with a level-1 heading and two plain paragraphs.
///
/// Produces:
/// ```text
/// Introduction    (heading level 1)
/// First paragraph.
/// Second paragraph.
/// ```
pub fn heading_and_paragraphs_content_xml(version: &str) -> Vec<u8> {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <office:document-content office:version=\"{version}\" \
         xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
         xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">\
         <office:automatic-styles/>\
         <office:body><office:text>\
         <text:h text:outline-level=\"1\">Introduction</text:h>\
         <text:p>First paragraph.</text:p>\
         <text:p>Second paragraph.</text:p>\
         </office:text></office:body>\
         </office:document-content>"
    )
    .into_bytes()
}

/// `content.xml` with a single paragraph and **no** `office:version`
/// attribute — valid for ODF 1.1 documents.
pub fn v1_1_content_xml() -> Vec<u8> {
    b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-content \
      xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
      xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">\
      <office:automatic-styles/>\
      <office:body><office:text>\
      <text:p>Hello world.</text:p>\
      </office:text></office:body>\
      </office:document-content>"
        .to_vec()
}
