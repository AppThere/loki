// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! ODF conformance integration tests (part 2) — line-height and spacing.
//!
//! Covers `style:line-height-at-least` and `fo:line-height` percentage
//! through the full ODT import pipeline. [MS-OODF] §3.1.

mod helpers;

use std::io::Cursor;

use loki_doc_model::style::catalog::StyleId;
use loki_doc_model::style::props::para_props::LineHeight;
use loki_odf::odt::import::{OdtImportOptions, OdtImporter};

// ── Line-height conformance ───────────────────────────────────────────────────

/// ODF `style:line-height-at-least="14pt"` on a paragraph style must resolve
/// to `LineHeight::AtLeast(p)` with `p ≈ 14.0 pt` after a full import.
/// [MS-OODF] §3.1 (`style:line-height-at-least` is supported by Word).
#[test]
fn odf7_line_height_at_least_maps_to_at_least() {
    let styles = br#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-styles office:version="1.2"
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0">
<office:automatic-styles/>
<office:styles>
  <style:style style:name="LhaPara" style:family="paragraph">
    <style:paragraph-properties style:line-height-at-least="14pt"/>
  </style:style>
</office:styles>
<office:master-styles/>
</office:document-styles>"#;

    let content = br#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content office:version="1.2"
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
<office:automatic-styles/>
<office:body><office:text>
  <text:p text:style-name="LhaPara">Test paragraph.</text:p>
</office:text></office:body>
</office:document-content>"#;

    let zip = helpers::build_odt_zip(content, styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("style:line-height-at-least should import without error");

    let resolved = result
        .document
        .styles
        .resolve_para(&StyleId::new("LhaPara"))
        .expect("LhaPara must be present in the style catalog");

    assert!(
        matches!(resolved.line_height, Some(LineHeight::AtLeast(p)) if (p.value() - 14.0).abs() < 0.5),
        "expected LineHeight::AtLeast(≈14 pt), got {:?}",
        resolved.line_height
    );
}

/// ODF `fo:line-height="150%"` on a paragraph style must resolve to
/// `LineHeight::Multiple(m)` with `m ≈ 1.5` after a full import.
/// ODF §6.7 (`fo:line-height` percentage format).
#[test]
fn odf8_fo_line_height_percentage_maps_to_multiple() {
    let styles = br#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-styles office:version="1.2"
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
  xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0">
<office:automatic-styles/>
<office:styles>
  <style:style style:name="LhPct" style:family="paragraph">
    <style:paragraph-properties fo:line-height="150%"/>
  </style:style>
</office:styles>
<office:master-styles/>
</office:document-styles>"#;

    let content = br#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content office:version="1.2"
  xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
  xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0">
<office:automatic-styles/>
<office:body><office:text>
  <text:p text:style-name="LhPct">Test paragraph.</text:p>
</office:text></office:body>
</office:document-content>"#;

    let zip = helpers::build_odt_zip(content, styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("fo:line-height percentage should import without error");

    let resolved = result
        .document
        .styles
        .resolve_para(&StyleId::new("LhPct"))
        .expect("LhPct must be present in the style catalog");

    let mult = match resolved.line_height {
        Some(LineHeight::Multiple(m)) => m,
        other => panic!("expected LineHeight::Multiple, got {other:?}"),
    };
    assert!(
        (mult - 1.5).abs() < 0.01,
        "fo:line-height=150% should map to Multiple(1.5), got {mult}"
    );
}
