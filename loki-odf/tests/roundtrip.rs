// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for the ODT import pipeline:
//! ZIP → package reader → XML readers → mappers → `loki_doc_model::Document`.
//!
//! Each test builds a minimal ODT archive in memory, imports it, and asserts
//! properties of the resulting document.

mod helpers;

use std::io::Cursor;

use loki_doc_model::content::block::Block;
use loki_odf::odt::import::{OdtImporter, OdtImportOptions};
use loki_odf::version::OdfVersion;

// ── Test: heading and paragraphs ───────────────────────────────────────────────

/// A document with a level-1 heading and two paragraphs should produce:
///
/// - `source_version = OdfVersion::V1_2`
/// - `sections[0].blocks[0]` = `Block::Heading(1, …)`
/// - `sections[0].blocks[1]` = `Block::StyledPara(…)`
/// - `sections[0].blocks[2]` = `Block::StyledPara(…)`
#[test]
fn roundtrip_odt_heading_and_paragraphs() {
    let content = helpers::heading_and_paragraphs_content_xml("1.2");
    let styles = helpers::empty_styles_xml("1.2");
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("import should succeed");

    assert_eq!(
        result.source_version,
        OdfVersion::V1_2,
        "detected version should be V1_2"
    );
    assert!(
        result.warnings.is_empty(),
        "unexpected warnings: {:?}",
        result.warnings
    );

    let doc = &result.document;
    assert_eq!(doc.sections.len(), 1, "document should have exactly one section");

    let blocks = &doc.sections[0].blocks;
    assert!(
        blocks.len() >= 3,
        "expected ≥ 3 blocks (heading + 2 paragraphs), got {}",
        blocks.len()
    );

    // Block 0 must be a level-1 heading with non-empty inline content
    match &blocks[0] {
        Block::Heading(level, _, inlines) => {
            assert_eq!(*level, 1, "heading level should be 1");
            assert!(!inlines.is_empty(), "heading inlines should not be empty");
        }
        other => panic!("expected Block::Heading(1, …), got {:?}", other),
    }

    // Blocks 1 and 2 must be styled paragraphs
    assert!(
        matches!(blocks[1], Block::StyledPara(_)),
        "block[1] should be StyledPara, got {:?}",
        blocks[1]
    );
    assert!(
        matches!(blocks[2], Block::StyledPara(_)),
        "block[2] should be StyledPara, got {:?}",
        blocks[2]
    );
}

// ── Test: ODF 1.1 version preserved ───────────────────────────────────────────

/// A document with no `office:version` attribute (valid ODF 1.1) must be
/// detected as [`OdfVersion::V1_1`] and the round-trip version stored in
/// `document.source.version` must be `"1.1"`.
#[test]
fn roundtrip_version_preserved_1_1() {
    let content = helpers::v1_1_content_xml();
    // styles.xml without a version attribute (ODF 1.1 style)
    let styles = helpers::empty_styles_xml("");
    let zip = helpers::build_odt_zip(&content, &styles, None);

    let result = OdtImporter::new(OdtImportOptions::default())
        .run(Cursor::new(zip))
        .expect("import should succeed for ODF 1.1 document");

    assert_eq!(
        result.source_version,
        OdfVersion::V1_1,
        "absent office:version should be detected as V1_1"
    );

    let source = result
        .document
        .source
        .as_ref()
        .expect("document.source must be Some");
    assert_eq!(
        source.version.as_deref(),
        Some("1.1"),
        "document.source.version must be \"1.1\""
    );
    assert_eq!(source.format, "odf", "document.source.format must be \"odf\"");
}
