// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 AppThere Loki contributors

//! Translates intermediate OOXML model types → [`loki_doc_model`] types.
//!
//! This is the second step of the two-step DOCX import pipeline:
//!
//! 1. **XML → intermediate model** (`reader` layer)
//! 2. **Intermediate model → [`loki_doc_model`]** (this layer)
//!
//! Entry point: [`map_document`].
//!
//! See [`mapping_plan`] for the full intermediate-model inventory, doc-model
//! inventory, and mapping decision tables.

// ══════════════════════════════════════════════════════════════════════════════
// Module declarations
// ══════════════════════════════════════════════════════════════════════════════

pub(crate) mod mapping_plan;

pub mod error;
pub use error::MapperError;

pub(crate) mod document;
pub(crate) mod images;
pub(crate) mod inline;
pub(crate) mod numbering;
pub(crate) mod paragraph;
pub(crate) mod props;
pub(crate) mod styles;
pub(crate) mod table;

// DocxSettings.even_and_odd_headers is now wired through map_document (Session 7).
// DocxSettings.default_tab_stop and title_pg remain unused pending further work.

// ══════════════════════════════════════════════════════════════════════════════
// Public entry point
// ══════════════════════════════════════════════════════════════════════════════

use loki_doc_model::document::Document;
use loki_opc::Package;

use crate::docx::import::{DocxImportOptions, parse_and_map_package};

/// Maps an OOXML OPC [`Package`] to a format-neutral [`Document`].
///
/// This is the primary public mapper entry point. It handles both the XML
/// parsing step (reading DOCX parts from the package) and the model-mapping
/// step (translating the intermediate model to [`loki_doc_model`]).
///
/// Non-fatal import warnings (unresolved relationships, unsupported features,
/// etc.) are discarded. For access to warnings, use
/// [`crate::docx::import::DocxImporter::run`] instead.
///
/// # Errors
///
/// Returns [`MapperError::Pipeline`] if the package is missing a required
/// part (e.g. the `officeDocument` relationship), if a mandatory XML part
/// cannot be parsed, or if an OPC-level error occurs. Optional or
/// enrichment-only parts (styles, numbering, footnotes) map to defaults
/// rather than erroring when absent.
///
/// Returns [`MapperError::MissingRequiredElement`] if a required OOXML
/// element is absent in the intermediate model.
///
/// Returns [`MapperError::InvalidValue`] if an element carries a value that
/// is structurally invalid.
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use loki_ooxml::docx::mapper::map_document;
/// use loki_ooxml::docx::import::DocxImportOptions;
/// use loki_opc::Package;
///
/// let file = File::open("document.docx")?;
/// let package = Package::open(file)?;
/// let doc = map_document(&package, &DocxImportOptions::default())?;
/// assert!(!doc.sections.is_empty());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn map_document(
    package: &Package,
    options: &DocxImportOptions,
) -> Result<Document, MapperError> {
    let (doc, _warnings) = parse_and_map_package(package, options)?;
    Ok(doc)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
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
}
