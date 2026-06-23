// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Tests for `mod`.

use super::*;
use loki_opc::relationships::{Relationship, TargetMode};
use loki_opc::{PartData, PartName};

const REL_OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const MEDIA_TYPE_DOCUMENT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";

/// Builds a minimal in-memory DOCX OPC package programmatically.
///
/// Contains only the parts needed by `map_document`: the package-level
/// `officeDocument` relationship and a valid `word/document.xml`.
fn make_package(doc_xml: &[u8]) -> Package {
    let mut pkg = Package::new();
    let part_name = PartName::new("/word/document.xml").unwrap();
    pkg.set_part(
        part_name,
        PartData::new(doc_xml.to_vec(), MEDIA_TYPE_DOCUMENT),
    );
    pkg.relationships_mut()
        .add(Relationship {
            id: "rId1".into(),
            rel_type: REL_OFFICE_DOCUMENT.into(),
            target: "/word/document.xml".into(),
            target_mode: TargetMode::Internal,
        })
        .unwrap();
    pkg
}

// ── Round-trip test ───────────────────────────────────────────────────────

#[test]
fn round_trip_minimal_document() {
    let package = make_package(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Hello, world!</w:t></w:r></w:p>
    <w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr>
  </w:body>
</w:document>"#,
    );
    let doc = map_document(&package, &DocxImportOptions::default())
        .expect("map_document must succeed for a minimal package");

    assert!(!doc.sections.is_empty(), "at least one section expected");
    let blocks = &doc.sections[0].blocks;
    assert!(!blocks.is_empty(), "paragraph should be present");

    use loki_doc_model::content::block::Block;
    assert!(
        matches!(blocks[0], Block::StyledPara(_)),
        "first block should be StyledPara, got {:?}",
        blocks[0]
    );
}

// ── Optional absent: no styles part → empty catalog, no error ────────────

#[test]
fn missing_styles_part_uses_defaults() {
    let package = make_package(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body/>
</w:document>"#,
    );
    let doc = map_document(&package, &DocxImportOptions::default())
        .expect("missing styles part should not error");
    // No styles were loaded; catalog is empty.
    assert!(doc.styles.paragraph_styles.is_empty());
}

// ── MapperError variants display correctly ────────────────────────────────

#[test]
fn missing_required_element_message() {
    let e = MapperError::MissingRequiredElement { element: "w:body" };
    assert!(e.to_string().contains("w:body"));
}

#[test]
fn invalid_value_message() {
    let e = MapperError::InvalidValue {
        element: "w:pgSz",
        detail: "width must be positive".into(),
    };
    let s = e.to_string();
    assert!(s.contains("w:pgSz"));
    assert!(s.contains("width must be positive"));
}

// ── A4 defaults when no sectPr present ────────────────────────────────────

#[test]
fn no_sect_pr_yields_a4_layout() {
    let package = make_package(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body/>
</w:document>"#,
    );
    let doc = map_document(&package, &DocxImportOptions::default()).unwrap();
    assert_eq!(doc.sections.len(), 1);
    let sz = &doc.sections[0].layout.page_size;
    // A4: 595.28 × 841.89 pt — allow ±0.1 pt tolerance.
    assert!(
        (sz.width.value() - 595.28).abs() < 0.1,
        "A4 width expected, got {}",
        sz.width.value()
    );
    assert!(
        (sz.height.value() - 841.89).abs() < 0.1,
        "A4 height expected, got {}",
        sz.height.value()
    );
}

// ── Pipeline error for missing officeDocument relationship ────────────────

#[test]
fn missing_office_document_rel_yields_pipeline_error() {
    // An empty package has no officeDocument relationship.
    let pkg = Package::new();
    let result = map_document(&pkg, &DocxImportOptions::default());
    assert!(result.is_err());
    assert!(
        matches!(result.unwrap_err(), MapperError::Pipeline(_)),
        "expected Pipeline error variant"
    );
}
