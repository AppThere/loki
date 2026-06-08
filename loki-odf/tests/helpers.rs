// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Minimal ODT package builder for integration tests.
//!
//! [`build_odt_zip`] produces a valid ODF ZIP archive from raw XML byte
//! slices. The helper XML constructors (`heading_and_paragraphs_content_xml`,
//! etc.) produce well-formed `content.xml` / `styles.xml` fixtures.

use std::io::{Cursor, Write};
use zip::CompressionMethod;
use zip::write::{FileOptions, ZipWriter};

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
pub fn build_odt_zip(content_xml: &[u8], styles_xml: &[u8], meta_xml: Option<&[u8]>) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));

    // mimetype must be first and stored (uncompressed)
    let stored = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
    zip.start_file("mimetype", stored).unwrap();
    zip.write_all(MIME_ODT.as_bytes()).unwrap();

    let deflated = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);

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

/// Build an in-memory ODF ZIP archive that intentionally omits `content.xml`.
///
/// Used by malformed-input tests to verify the importer returns `Err` rather
/// than panicking when a mandatory package entry is absent.
pub fn build_odt_zip_no_content(styles_xml: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(Cursor::new(&mut buf));

    let stored = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
    zip.start_file("mimetype", stored).unwrap();
    zip.write_all(MIME_ODT.as_bytes()).unwrap();

    let deflated = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);

    zip.start_file("META-INF/manifest.xml", deflated).unwrap();
    zip.write_all(MANIFEST).unwrap();

    zip.start_file("styles.xml", deflated).unwrap();
    zip.write_all(styles_xml).unwrap();

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

/// `content.xml` with a heading, a paragraph with bold/italic spans, and a
/// bullet list — used by the `round_trip` smoke tests.
///
/// Produces:
/// ```text
/// Introduction          (heading level 1)
/// Bold text and italic. (styled paragraph)
/// • Item one            (BulletList)
/// • Item two
/// ```
pub fn rich_fixture_content_xml(version: &str) -> Vec<u8> {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <office:document-content office:version=\"{version}\" \
         xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
         xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" \
         xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\">\
         <office:automatic-styles>\
           <style:style style:name=\"bold_span\" style:family=\"text\" \
             xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\">\
             <style:text-properties fo:font-weight=\"bold\"/>\
           </style:style>\
           <style:style style:name=\"italic_span\" style:family=\"text\" \
             xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\">\
             <style:text-properties fo:font-style=\"italic\"/>\
           </style:style>\
         </office:automatic-styles>\
         <office:body><office:text>\
         <text:h text:outline-level=\"1\">Introduction</text:h>\
         <text:p>\
           <text:span text:style-name=\"bold_span\">Bold text</text:span>\
           <text:s/>\
           and\
           <text:s/>\
           <text:span text:style-name=\"italic_span\">italic text</text:span>.\
         </text:p>\
         <text:list>\
           <text:list-item><text:p>Item one</text:p></text:list-item>\
           <text:list-item><text:p>Item two</text:p></text:list-item>\
         </text:list>\
         </office:text></office:body>\
         </office:document-content>"
    )
    .into_bytes()
}

/// `styles.xml` that declares an A4 page layout, a heading style named
/// `"Heading_20_1"` (LibreOffice encoding of "Heading 1") with 18 pt bold
/// Liberation Sans, and a body style with `fo:font-family`.
///
/// Used by gap-coverage tests for page size, heading style resolution, and
/// font property propagation.
pub fn rich_styles_xml() -> Vec<u8> {
    b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-styles \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" \
        xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\">\
      <office:automatic-styles>\
        <style:page-layout style:name=\"pm1\">\
          <style:page-layout-properties \
            fo:page-width=\"21cm\" fo:page-height=\"29.7cm\" \
            fo:margin-top=\"2.54cm\" fo:margin-bottom=\"2.54cm\" \
            fo:margin-left=\"3.17cm\" fo:margin-right=\"3.17cm\"/>\
        </style:page-layout>\
      </office:automatic-styles>\
      <office:styles>\
        <style:style style:name=\"Heading_20_1\" style:display-name=\"Heading 1\" \
          style:family=\"paragraph\">\
          <style:paragraph-properties fo:margin-left=\"0cm\" fo:text-indent=\"0cm\"/>\
          <style:text-properties style:font-name=\"Liberation Sans\" \
            fo:font-size=\"18pt\" fo:font-weight=\"bold\"/>\
        </style:style>\
        <style:style style:name=\"BodyText\" style:family=\"paragraph\">\
          <style:text-properties fo:font-family=\"Liberation Serif\" \
            fo:font-size=\"11pt\"/>\
        </style:style>\
      </office:styles>\
      <office:master-styles>\
        <style:master-page style:name=\"Standard\" \
          style:page-layout-name=\"pm1\"/>\
      </office:master-styles>\
      </office:document-styles>"
        .to_vec()
}

/// `content.xml` that uses the styles defined in [`rich_styles_xml`]:
/// a heading at level 1 with `text:style-name="Heading_20_1"` and a body
/// paragraph with `text:style-name="BodyText"` and an indented paragraph.
pub fn rich_content_xml_with_styles() -> Vec<u8> {
    b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-content \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" \
        xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\">\
      <office:automatic-styles>\
        <style:style style:name=\"P1\" style:family=\"paragraph\">\
          <style:paragraph-properties fo:margin-left=\"1cm\" fo:text-indent=\"0cm\"/>\
        </style:style>\
      </office:automatic-styles>\
      <office:body><office:text>\
        <text:h text:outline-level=\"1\" text:style-name=\"Heading_20_1\"\
          >Chapter One</text:h>\
        <text:p text:style-name=\"BodyText\">Body paragraph text.</text:p>\
        <text:p text:style-name=\"P1\">Indented paragraph.</text:p>\
      </office:text></office:body>\
      </office:document-content>"
        .to_vec()
}

/// `styles.xml` with paragraph styles for border, tab stops, and background
/// color tests (ODF-1, ODF-2, ODF-3).
pub fn para_props_styles_xml() -> Vec<u8> {
    b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-styles \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" \
        xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\">\
      <office:automatic-styles/>\
      <office:styles>\
        <style:style style:name=\"BorderPara\" style:family=\"paragraph\">\
          <style:paragraph-properties \
            fo:border=\"1pt solid #000000\" \
            fo:padding=\"2pt\"/>\
        </style:style>\
        <style:style style:name=\"TabPara\" style:family=\"paragraph\">\
          <style:paragraph-properties>\
            <style:tab-stops>\
              <style:tab-stop style:position=\"2cm\" style:type=\"left\"/>\
              <style:tab-stop style:position=\"8cm\" style:type=\"right\"/>\
            </style:tab-stops>\
          </style:paragraph-properties>\
        </style:style>\
        <style:style style:name=\"BgPara\" style:family=\"paragraph\">\
          <style:paragraph-properties fo:background-color=\"#FFFFCC\"/>\
        </style:style>\
      </office:styles>\
      <office:master-styles/>\
      </office:document-styles>"
        .to_vec()
}

/// `content.xml` using the styles from [`para_props_styles_xml`]:
/// one paragraph each for border, tab stops, and background colour.
pub fn para_props_content_xml() -> Vec<u8> {
    b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-content \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" \
        xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\">\
      <office:automatic-styles/>\
      <office:body><office:text>\
        <text:p text:style-name=\"BorderPara\">Bordered paragraph.</text:p>\
        <text:p text:style-name=\"TabPara\">Tab stop paragraph.</text:p>\
        <text:p text:style-name=\"BgPara\">Background colour paragraph.</text:p>\
      </office:text></office:body>\
      </office:document-content>"
        .to_vec()
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

/// `styles.xml` with a master page containing all three header/footer variants:
/// default, first-page, and even-page. The default header includes a
/// `text:page-number` field to exercise field code parsing in headers.
///
/// Used by the header/footer round-trip and layout integration tests.
pub fn hf_styles_xml() -> Vec<u8> {
    b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-styles \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" \
        xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\">\
      <office:automatic-styles>\
        <style:page-layout style:name=\"pm1\">\
          <style:page-layout-properties \
            fo:page-width=\"21cm\" fo:page-height=\"29.7cm\" \
            fo:margin-top=\"2.54cm\" fo:margin-bottom=\"2.54cm\" \
            fo:margin-left=\"3.17cm\" fo:margin-right=\"3.17cm\"/>\
        </style:page-layout>\
      </office:automatic-styles>\
      <office:styles/>\
      <office:master-styles>\
        <style:master-page style:name=\"Standard\" \
            style:page-layout-name=\"pm1\">\
          <style:header>\
            <text:p>Test Header Page \
              <text:page-number text:select-page=\"current\">1</text:page-number>\
            </text:p>\
          </style:header>\
          <style:footer>\
            <text:p>Test Footer</text:p>\
          </style:footer>\
          <style:header-first>\
            <text:p>First Page Header</text:p>\
          </style:header-first>\
          <style:footer-first>\
            <text:p>First Page Footer</text:p>\
          </style:footer-first>\
          <style:header-left>\
            <text:p>Even Page Header</text:p>\
          </style:header-left>\
          <style:footer-left>\
            <text:p>Even Page Footer</text:p>\
          </style:footer-left>\
        </style:master-page>\
      </office:master-styles>\
      </office:document-styles>"
        .to_vec()
}

/// `styles.xml` for the multi-master-page fixture.
///
/// Declares two page layouts (portrait A4 and landscape A4) and two master
/// pages ("Standard" and "Landscape") each referencing a different layout.
/// Each master page has a distinct header paragraph so the import can be
/// verified by header content.
pub fn multi_master_styles_xml() -> Vec<u8> {
    b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-styles \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" \
        xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\">\
      <office:automatic-styles>\
        <style:page-layout style:name=\"pm-portrait\">\
          <style:page-layout-properties \
            fo:page-width=\"21cm\" fo:page-height=\"29.7cm\" \
            fo:margin-top=\"2cm\" fo:margin-bottom=\"2cm\" \
            fo:margin-left=\"2.5cm\" fo:margin-right=\"2.5cm\"/>\
        </style:page-layout>\
        <style:page-layout style:name=\"pm-landscape\">\
          <style:page-layout-properties \
            fo:page-width=\"29.7cm\" fo:page-height=\"21cm\" \
            fo:margin-top=\"2cm\" fo:margin-bottom=\"2cm\" \
            fo:margin-left=\"2.5cm\" fo:margin-right=\"2.5cm\"/>\
        </style:page-layout>\
      </office:automatic-styles>\
      <office:styles/>\
      <office:master-styles>\
        <style:master-page style:name=\"Standard\" \
            style:page-layout-name=\"pm-portrait\">\
          <style:header><text:p>Portrait Header</text:p></style:header>\
        </style:master-page>\
        <style:master-page style:name=\"Landscape\" \
            style:page-layout-name=\"pm-landscape\">\
          <style:header><text:p>Landscape Header</text:p></style:header>\
        </style:master-page>\
      </office:master-styles>\
      </office:document-styles>"
        .to_vec()
}

/// `content.xml` for the multi-master-page fixture.
///
/// The first paragraph has `text:style-name="Standard"` (portrait, initial
/// master page). The second paragraph uses `text:style-name="LandscapeStyle"`,
/// which carries `style:master-page-name="Landscape"` — this triggers a master
/// page transition and should produce a second document section.
pub fn multi_master_content_xml() -> Vec<u8> {
    b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-content \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" \
        xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\">\
      <office:automatic-styles>\
        <style:style style:name=\"LandscapeStyle\" style:family=\"paragraph\" \
          style:master-page-name=\"Landscape\"/>\
      </office:automatic-styles>\
      <office:body><office:text>\
        <text:p text:style-name=\"Standard\">Portrait paragraph.</text:p>\
        <text:p text:style-name=\"LandscapeStyle\">Landscape paragraph.</text:p>\
      </office:text></office:body>\
      </office:document-content>"
        .to_vec()
}

/// Minimal `content.xml` for use with [`hf_styles_xml`].
pub fn hf_content_xml() -> Vec<u8> {
    b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-content \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">\
      <office:automatic-styles/>\
      <office:body><office:text>\
        <text:p>Body paragraph one.</text:p>\
        <text:p>Body paragraph two.</text:p>\
      </office:text></office:body>\
      </office:document-content>"
        .to_vec()
}

/// `styles.xml` for the cell-properties fixture.
///
/// Defines a single A4 page layout, a master page, and a `StyledCell`
/// table-cell style carrying padding, vertical-align, background-color,
/// and border shorthand.
pub fn cell_props_styles_xml() -> Vec<u8> {
    b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-styles \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" \
        xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\">\
      <office:automatic-styles>\
        <style:page-layout style:name=\"pm1\">\
          <style:page-layout-properties \
            fo:page-width=\"21cm\" fo:page-height=\"29.7cm\" \
            fo:margin-top=\"2.54cm\" fo:margin-bottom=\"2.54cm\" \
            fo:margin-left=\"3.17cm\" fo:margin-right=\"3.17cm\"/>\
        </style:page-layout>\
      </office:automatic-styles>\
      <office:styles>\
        <style:style style:name=\"StyledCell\" style:family=\"table-cell\">\
          <style:table-cell-properties \
            fo:padding=\"0.2cm\" \
            style:vertical-align=\"middle\" \
            fo:background-color=\"#FFFF00\" \
            fo:border=\"0.06pt solid #000000\"/>\
        </style:style>\
        <style:style style:name=\"BottomCell\" style:family=\"table-cell\">\
          <style:table-cell-properties \
            fo:padding-top=\"0.1cm\" fo:padding-bottom=\"0.3cm\" \
            fo:padding-left=\"0.2cm\" fo:padding-right=\"0.2cm\" \
            style:vertical-align=\"bottom\"/>\
        </style:style>\
      </office:styles>\
      <office:master-styles>\
        <style:master-page style:name=\"Standard\" \
          style:page-layout-name=\"pm1\"/>\
      </office:master-styles>\
      </office:document-styles>"
        .to_vec()
}

/// `content.xml` for the cell-properties fixture.
///
/// A single table with two cells: the first references `StyledCell`
/// (shorthand padding, middle alignment, yellow background, black border)
/// and the second references `BottomCell` (per-edge padding, bottom alignment).
pub fn cell_props_content_xml() -> Vec<u8> {
    b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
      <office:document-content \
        office:version=\"1.2\" \
        xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" \
        xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" \
        xmlns:table=\"urn:oasis:names:tc:opendocument:xmlns:table:1.0\" \
        xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\">\
      <office:automatic-styles/>\
      <office:body><office:text>\
        <table:table table:name=\"CellPropsTable\">\
          <table:table-column/>\
          <table:table-column/>\
          <table:table-row>\
            <table:table-cell table:style-name=\"StyledCell\" \
              office:value-type=\"string\" \
              xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\">\
              <text:p>Styled Cell</text:p>\
            </table:table-cell>\
            <table:table-cell table:style-name=\"BottomCell\" \
              office:value-type=\"string\" \
              xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\">\
              <text:p>Bottom Cell</text:p>\
            </table:table-cell>\
          </table:table-row>\
        </table:table>\
      </office:text></office:body>\
      </office:document-content>"
        .to_vec()
}
