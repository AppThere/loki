// Copyright 2026 AppThere Loki contributors
// SPDX-License-Identifier: Apache-2.0

//! Top-level DOCX package assembly.
//!
//! Assembles the OPC package from the serialized parts produced by the other
//! modules in `write/`, wires up relationships and content types, and writes
//! the ZIP to the caller-supplied writer.
//!
//! ADR-0007: export via `loki-opc`'s `Package::write`.

use std::io::{Seek, Write};

use loki_doc_model::document::Document;
use loki_opc::part::{PartData, PartName};
use loki_opc::relationships::{Relationship, TargetMode};
use loki_opc::Package;

use crate::docx::write::document::write_document_xml;
use crate::docx::write::numbering::{write_numbering_xml, NumberingState};
use crate::docx::write::styles::write_styles_xml;
use crate::error::OoxmlError;

// ── OPC relationship type URIs ───────────────────────────────────────────────

const REL_OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const REL_STYLES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
const REL_NUMBERING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";

// ── OOXML media types ────────────────────────────────────────────────────────

const MT_DOCUMENT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const MT_STYLES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
const MT_NUMBERING: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";

/// Assembles a complete `.docx` package from `doc` and writes it to `writer`.
pub(crate) fn assemble_docx(
    doc: &Document,
    writer: impl Write + Seek,
) -> Result<(), OoxmlError> {
    // ── Step 1: Build styles.xml ─────────────────────────────────────────
    let styles_bytes = write_styles_xml(&doc.styles);

    // ── Step 2: Build document.xml (and collect numbering state) ─────────
    let mut num_state = NumberingState::new();
    let document_bytes =
        write_document_xml(&doc.sections, &doc.styles, &mut num_state);

    // ── Step 3: Build numbering.xml (only if lists exist) ────────────────
    let numbering_bytes = if num_state.is_empty() {
        None
    } else {
        Some(write_numbering_xml(&num_state))
    };

    // ── Step 4: Assemble OPC package ─────────────────────────────────────
    let mut pkg = Package::new();

    // Part names.
    let doc_part = PartName::new("/word/document.xml").map_err(OoxmlError::Opc)?;
    let styles_part = PartName::new("/word/styles.xml").map_err(OoxmlError::Opc)?;
    let numbering_part = PartName::new("/word/numbering.xml").map_err(OoxmlError::Opc)?;

    // Insert parts.
    pkg.set_part(
        doc_part.clone(),
        PartData::new(document_bytes, MT_DOCUMENT),
    );
    pkg.set_part(
        styles_part.clone(),
        PartData::new(styles_bytes, MT_STYLES),
    );
    if let Some(nb) = numbering_bytes {
        pkg.set_part(
            numbering_part.clone(),
            PartData::new(nb, MT_NUMBERING),
        );
    }

    // ── Package-level relationship: /_rels/.rels → word/document.xml ─────
    pkg.relationships_mut()
        .add(Relationship {
            id: "rId1".to_string(),
            rel_type: REL_OFFICE_DOCUMENT.to_string(),
            target: "word/document.xml".to_string(),
            target_mode: TargetMode::Internal,
        })
        .map_err(OoxmlError::Opc)?;

    // ── Document-level relationships: word/_rels/document.xml.rels ───────
    pkg.part_relationships_mut(&doc_part)
        .add(Relationship {
            id: "rId1".to_string(),
            rel_type: REL_STYLES.to_string(),
            target: "styles.xml".to_string(),
            target_mode: TargetMode::Internal,
        })
        .map_err(OoxmlError::Opc)?;

    // numbering (only if we inserted the part — tracked via num_state)
    if !num_state.is_empty() {
        pkg.part_relationships_mut(&doc_part)
            .add(Relationship {
                id: "rId2".to_string(),
                rel_type: REL_NUMBERING.to_string(),
                target: "numbering.xml".to_string(),
                target_mode: TargetMode::Internal,
            })
            .map_err(OoxmlError::Opc)?;
    }

    // ── Content types ─────────────────────────────────────────────────────
    let ct = pkg.content_type_map_mut();
    ct.add_default(
        "rels",
        "application/vnd.openxmlformats-package.relationships+xml",
    );
    ct.add_default("xml", "application/xml");
    ct.add_override(&doc_part, MT_DOCUMENT);
    ct.add_override(&styles_part, MT_STYLES);
    if !num_state.is_empty() {
        ct.add_override(&numbering_part, MT_NUMBERING);
    }

    // ── Step 5: Write ZIP ─────────────────────────────────────────────────
    pkg.write(writer).map_err(OoxmlError::Opc)
}
